# M7 W5-H-PRNG-1: port the retail PRNG

Status: implemented and tested. HEAD b5a2d56. Files (write scope only):
crates/wolf-vm/src/rng.rs, crates/wolf-vm/tests/rng_retail.rs, docs/research/M7_PRNG.md.
No commit/push; the Lead integrates.

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

## 2. Algorithm (proven)

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
table) seeds the single LCG state from a (random_a); random_b and the 28,936-byte
table are the same stream's later outputs, so they are accepted for API/source
fidelity but do not need to be replayed separately.

Range mapping used by 0x6000 (see §4 for proven vs inferred):

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

range(1, 10) from seed 22227, first 8 results: 3, 1, 2, 6, 8, 10, 5, 4.
range(-20000, 20000) from seed 22227 over 3000 draws: min -19997, max 19976
(both tails reached, so the mapping uses the full signed span).

## 4. Proven vs inferred

PROVEN (from _wolfref/game_dat_random.c + FORMAT_SPEC §2.5):
- 32-bit single-word state; multiplier 0x343FD; increment 0x269EC3; mod 2^32.
- Output is the middle 15 bits: (state >> 16) & 0x7fff, range 0..=32767.
- The Game.dat static block (r1=22227, r2=25684, 28936 bytes 0..=127) is one
  stream of this LCG; each stored byte is (state >> 24) & 0x7f.
- The 0x6000 operator is a random range rand(min, max) (docs/OPCODE_SPEC.md §3.2,
  1,101 sites; operand shapes only make sense as a range).
- Range semantics: inclusive on both ends, out-of-order bounds swapped,
  zero-width returns the lower bound (docs/OPCODE_SPEC.md:248).

INFERRED (not decompiled; no engine source in _wolfref, only the seed tool):
- How one 15-bit draw maps onto [min, max]. This port uses multiply-high,
  (draw*(span+1)) >> 15. Rationale: a plain draw % (span+1) would cap any span
  wider than 32,767 at the low end (e.g. -20000..20000 could never exceed +12767,
  which is implausible for a symmetric damage float), so scaling across the span
  is the safer reconstruction. The exact rounding (shift amount / bias) is the
  one open question.
- Whether the retail operator consumes the stored byte table sequentially or
  runs the LCG live. This port runs the LCG live seeded from random_a. Because
  the table is the same LCG stream, a fixed Game.dat yields a fixed, replayable
  sequence either way; byte-for-byte equality with the original engine is the
  unproven part.
- The seed actually used at runtime (random_a assumed; random_b unused here).

Impact: gameplay randomness is now deterministic and reproducible from a seed
(same seed -> same battle rolls). If the engine's range operator used a different
reduction, the values differ but the sequence of underlying draws is identical,
so a later correction is a one-line change in WolfRng::range.

## 5. Tests

crates/wolf-vm/src/rng.rs (unit): matches_the_msvc_rand_sequence_for_seed_1,
matches_the_game_seed_vectors, range_stays_inside_the_bounds,
reversed_bounds_are_swapped, a_degenerate_range_is_a_constant,
the_sequence_is_deterministic_and_not_constant, different_seeds_diverge.

crates/wolf-vm/tests/rng_retail.rs (public API): same_seed_produces_the_same_sequence,
different_seeds_produce_different_sequences, matches_the_msvc_rand_vectors,
matches_the_game_seed_vectors, from_game_seeds_from_the_first_static_word,
range_is_inclusive_and_stays_inside_the_bounds, a_wide_range_uses_both_halves,
range_draws_are_exact_vectors, reversed_bounds_are_swapped,
zero_width_ranges_do_not_panic, draws_counts_range_calls_not_raw_steps.

Command and result:

    cargo test -q -p wolf-vm
    -> unit: 87 passed; 0 failed
    -> rng_retail integration: 11 passed; 0 failed
    (exit 0)

## 6. Reproduce / residual work

- Re-run: cargo test -q -p wolf-vm
- To close the inferred gap, decompile Game.exe's random-operator routine (the
  one called for operator 0x6000) and read its range reduction, then align
  WolfRng::range. Everything upstream of that (state, constants, 15-bit draw) is
  already exact.
