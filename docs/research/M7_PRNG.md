# M7 W5-H-PRNG-1: port the retail PRNG

Status: **superseded in part by M8 W5-a** (`docs/research/M8_PRNG_RANGE.md`).
This document accurately describes the **MSVC C-runtime LCG**, but that stream
is the Game.dat **static-table / crypto** stream — it is NOT the generator that
opcode 121's `0x6000` operator calls. M8 decompiled the operator and showed it
calls a Park-Miller MINSTD generator (RVA 0x35810) and reduces its 31-bit draw
with a modulo law (RVA 0xaf7c0). `WolfRng::range` now uses that event stream;
`next_raw` keeps this MSVC stream for the static table. The multiply-high
"inferred" range mapping below was wrong and is retained only as history.

Original task record: implemented and tested at HEAD b5a2d56. Files (write scope
only): crates/wolf-vm/src/rng.rs, crates/wolf-vm/tests/rng_retail.rs,
docs/research/M7_PRNG.md. No commit/push; the Lead integrates.

## 0. Correction (M8)

The seed-recovery evidence in §1 proves the **static-table** block is one MSVC
LCG stream, and §1's `random_a = 22227` is an encoded draw, not the stream's
seed (M8 recovered the real seed state `0xe765b62e`). It says nothing about the
event RNG. M8's decompile of opcode 121 (RVA 0xaf7c0) shows the random case
calls RVA 0x35810 and never the MSVC routine; see M8_PRNG_RANGE §2-3. The
multiply-high range mapping proposed in §2/§4 is refuted by the decompiled
`div`-based reduction in M8_PRNG_RANGE §1.

## 1. Source evidence

The retail PRNG is fully documented by the bundled seed-recovery tool
_wolfref/game_dat_random.c. Its header comment (lines 5, 12-15) is the spec:

    #define RAND(a) ((a)*0x343fd+0x269ec3)
    // getNextRandom() {
    //     _seed = RAND(_seed)              // _seed*0x343fd+0x269ec3
    //     return ((_seed >> 16) & 0x7fff)
    // }

The verification loops in the same file prove the byte table is the same stream
(they test data bytes against ((cur >> 24) & 0x7f), lines 93 and 111):

    if (((cur >> 24) & 0x7f) != data[j]) { break; }   // game_dat_random.c:93

Corroborating facts already in the repo:

- docs/FORMAT_SPEC.md:260-267 (§2.5 static_randoms): block starts at 328,
  length = filesize - 328 = 28944 bytes; first 8 bytes are u4 r1 = 22227,
  u4 r2 = 25684; then 28936 bytes, all 0..=127; regenerated on every editor
  save; "same PRNG reproduces it".
- crates/wolf-format/src/gamedat.rs:87-90 reads random_a, random_b, and the
  remaining bytes as static_randoms; random_a == 22227, random_b == 25684
  (also asserted in crates/wolf-cli/src/main.rs:641-642).
- The operator that needs it: crates/wolf-vm/src/exec.rs:1672-1690 (opcode 121);
  when the binary-op nibble is 6 (0x6000) it calls rng.range(a, b)
  (exec.rs:1681-1682). The Vm owns pub rng: WolfRng (exec.rs:796), created with
  WolfRng::new() at exec.rs:822.

## 2. Algorithm (proven, for the static-table stream)

Scope: this algorithm describes the Game.dat static-random block and data-file
decryption only. It is **not** the event RNG behind 0x6000 (see §0).

State: a single 32-bit word _seed.

Step (each draw):

    state = state.wrapping_mul(0x343FD).wrapping_add(0x269EC3)   // mod 2^32
    draw  = (state >> 16) & 0x7FFF                              // 15-bit, 0..=32767

Constants: MULTIPLIER 0x343FD = 214013, INCREMENT 0x269EC3 = 2531011.
This is exactly the Microsoft C runtime rand()/srand() LCG. The canonical
external anchor: srand(1) yields 41, 18467, 6334, 26500, 19169, 15724, 11478,
29358, ... and the port reproduces it bit for bit (unit test
matches_the_msvc_rand_sequence_for_seed_1).

Seed for this build: DEFAULT_SEED = 22227 = Game.dat random_a. from_game(a, b,
table) seeds the single LCG state from a (random_a). M8 later showed random_a
and random_b are *encoded draws* of this stream, not its seed, and recovered the
real seed state 0xe765b62e; the API is kept for source/loader stability.

Range mapping proposed here (REJECTED by M8; kept as history):

    low, high = min <= max ? (min, max) : (max, min)
    span = high - low
    if span <= 0: return low                 // zero-width: constant, no draw
    value = (draw * (span + 1)) >> 15        // multiply-high, spans the whole range
    return low + value

## 3. Observed vectors (deterministic; pinned in tests)

seed = 1, first 8 draws (state, then (state>>16)&0x7fff):

    state: 2745024, 3357800067, 415139642, 3884216597, 3403800452, 1030492215, 752224798, 1924036713
    draw : 41, 18467, 6334, 26500, 19169, 15724, 11478, 29358

seed = 22227 (DEFAULT_SEED / random_a), first 5 draws:

    state: 464430666, 69489637, 2511435540, 3354364295, 3849641518
    draw : 7086, 1060, 5553, 18415, 25972

range(1, 10) from seed 22227, first 8 results under the rejected multiply-high
mapping: 3, 1, 2, 6, 8, 10, 5, 4. These are void; the landed modulo-law vectors
(for the Park-Miller event stream) are in M8_PRNG_RANGE §5.
range(-20000, 20000) from seed 22227 over 3000 draws under the rejected mapping:
min -19997, max 19976. These are void too: M8 showed the range operator uses a
modulo reduction of a 31-bit Park-Miller draw, not multiply-high of a 15-bit
MSVC draw.

## 4. Proven vs inferred (static-table scope)

PROVEN (from _wolfref/game_dat_random.c + FORMAT_SPEC §2.5):
- 32-bit single-word state; multiplier 0x343FD; increment 0x269EC3; mod 2^32.
- Output is the middle 15 bits: (state >> 16) & 0x7fff, range 0..=32767.
- The Game.dat static block (r1=22227, r2=25684, 28936 bytes 0..=127) is one
  stream of this LCG; each stored byte is (state >> 24) & 0x7f.
- This stream is the static-table/crypto stream. It is NOT called by the 0x6000
  operator (M8 decompile: RVA 0xaf7c0 calls RVA 0x35810 instead).

CORRECTED / REFUTED by M8:
- "The 0x6000 operator consumes this LCG's 15-bit draw" — false.
- "Range mapping is multiply-high (draw*(span+1))>>15" — false: retail does
  `low + draw % (abs(b-a)+1)` on a 31-bit Park-Miller draw.
- "random_a seeds the stream" — false: random_a/random_b are encoded draws;
  the recovered seed state is 0xe765b62e.
- "Zero-width consumes no draw" — false: the generator is called first, so it
  consumes one draw, and the result is the lower bound.
- Order of bounds is still handled as min + absolute difference (equivalent to
  swapping), which the decompile confirmed.

The landed event generator and range law are documented in
docs/research/M8_PRNG_RANGE.md (§1-5); the code API split is described there
too (`next_event_draw`/`range` for events, `next_raw` for the static table).

## 5. Tests

The M7 tests still pin the MSVC stream; the range tests were replaced by the
pinned modulo-law/event-stream vectors in M8:

crates/wolf-vm/src/rng.rs (unit, current): matches_the_msvc_rand_sequence_for_seed_1,
matches_the_game_seed_vectors, matches_the_park_miller_event_vectors,
the_rejection_loop_multiplies_again_above_the_ceiling,
a_zero_seed_reseeds_instead_of_sticking_at_zero,
range_stays_inside_the_bounds, reversed_bounds_are_swapped,
a_degenerate_range_is_a_constant_but_consumes_a_draw,
range_matches_the_pinned_modulo_law,
the_sequence_is_deterministic_and_not_constant, different_seeds_diverge.

crates/wolf-vm/tests/rng_retail.rs (public API, current, 15 tests): the MSVC
anchor/static-table tests plus the event-domain tests
(matches_the_park_miller_event_vectors,
the_event_draw_is_the_full_retail_domain_not_15_bits,
range_draws_are_exact_vectors_under_the_pinned_modulo_law,
range_agrees_with_the_law_formula_draw_for_draw,
zero_width_ranges_consume_exactly_one_draw,
an_extreme_range_uses_the_retail_32_bit_wrapping_modulus, ...).

Command and result after M8:

    cargo test -q -p wolf-vm
    -> unit: 91 passed; 0 failed
    -> rng_retail integration: 15 passed; 0 failed
    (exit 0)

## 6. Reproduce / residual work

- Re-run: cargo test -q -p wolf-vm
- The operator decompile is done (M8); the only remaining parity gap is the
  event generator's clock seed and same-save replay, see M8_PRNG_RANGE §6.
- The static-table/byte-parity question remains separate: the recovered seed
  state is 0xe765b62e, and `from_game` should not be read as "random_a seeds
  the stream".
