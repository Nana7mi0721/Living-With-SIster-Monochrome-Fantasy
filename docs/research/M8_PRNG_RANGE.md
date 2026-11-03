# M8 W5-a: retail range reduction for 121 / operator 0x6000 `rand(min,max)`

Status: **DONE (implementation landed).** The range law *and* the generator it
consumes are now in `crates/wolf-vm/src/rng.rs`: `WolfRng::next_event_draw` runs
the retail Park-Miller MINSTD stream from RVA 0x35810 and `WolfRng::range`
applies the pinned modulo law from RVA 0xaf7c0. The MSVC C LCG is kept only for
the Game.dat static-table stream (`next_raw`), matching §3's finding that it is
not the event source. No commit/push; the Lead integrates.

Production clock wiring landed later (2026-11-03): interactive `wolf-cli play`
seeds from the local clock through `resolve_session_event_seed`; the library
keeps `EVENT_SEED = 1` for deterministic tests and diagnostics, and
`WOLF_EVENT_SEED` replays a captured tick. See `docs/research/M9_PRNG_SEED.md`.
Byte-for-byte parity with one specific retail run still needs that run's
GetTickCount, an optional check rather than a functional gap; see §6.

## 1. Verdict

The retail operator applies a **modulo reduction** to a 31-bit Park-Miller
draw:

```text
low     = min(arg1, arg2)
modulus = abs(arg2 - arg1) + 1        // 32-bit wrapping subtraction/abs,
                                      // then +1 (never zero)
value   = low + (draw % modulus)      // unsigned division
```

* Inclusive on both ends.
* Out-of-order bounds are handled by `min` + absolute difference; equivalent
  to swapping.
* Zero-width range is constant, but **still consumes one draw** (the generator
  is called before the span is computed).
* Multiply-high (`draw*(span+1)>>15`) and double scaling are both **refuted**.

## 2. Evidence (actual retail binary)

Binaries examined (both already in the workspace, no download):

| file | sha256 | size |
|---|---|---|
| `work/runcopy/Game.exe` (the game's shipped runtime) | `98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea` | 7,329,792 |
| `work/w5a_ref/Game.exe` (generic 3.5 runtime extracted from `work/_wolked.zip`) | `150ee7ec59dcc2be794f8a0bbbf705deb2569ee61c4abbded1fb17c4ab0f5157` | 12,547,584 |

PE32, image base 0x400000. Addresses below are **RVAs**; file offset for
`.text` is `0x400 + (rva - 0x1000)`.

### 2.1 Operator handler and the random case

`work/runcopy/Game.exe`, handler function RVA `0xaea70` (opcode 121). Arg
access is 1-based (`push 4` = `arg[3]`, the operator word). It evaluates
`arg[1]`/`arg[2]` into `[ebp-0x1f0]`/`[ebp-0x1f4]`, then dispatches:

```text
0xaf6d0: mov  eax,[eax]          ; arg[3]
0xaf6d2: sar  eax,0xc
0xaf6d5: and  eax,0xf
0xaf6d8: cmp  eax,6
0xaf6db: ja   default
0xaf6dd: jmp  dword ptr [eax*4+0x4b0158]
```

Switch table (VA 0x4b0158, 7 entries):

| nibble | target RVA | operation |
|---|---|---|
| 0 | 0xaf6e4 | add |
| 1 | 0xaf737 | subtract |
| 2 | 0xaf74d | multiply |
| 3 | 0xaf769 | signed divide (divisor 0 -> 1) |
| 4 | 0xaf787 | signed modulo (divisor 0 -> 1) |
| 5 | 0xaf7ab | bitwise and |
| 6 | **0xaf7c0** | **random range** |

Case 6, verbatim (file offset 0xaebc0):

```text
0xaf7c0: xor  edx,edx
0xaf7c2: xor  ecx,ecx
0xaf7c4: call 0x35810            ; 31-bit Park-Miller draw (time-seeded)
0xaf7c9: mov  esi,eax            ; draw
0xaf7cb: mov  eax,[ebp-0x1f0]    ; arg1
0xaf7d1: sub  eax,[ebp-0x1f4]    ; arg1 - arg2  (32-bit)
0xaf7d7: cdq
0xaf7d8: mov  ecx,eax
0xaf7da: mov  eax,esi
0xaf7dc: xor  ecx,edx
0xaf7de: mov  esi,[ebp-0x1f0]    ; arg1
0xaf7e4: sub  ecx,edx            ; ecx = abs(arg1-arg2)
0xaf7e6: xor  edx,edx
0xaf7e8: inc  ecx                ; modulus = abs(diff)+1
0xaf7e9: div  ecx                ; unsigned; edx = draw % modulus
0xaf7eb: mov  ecx,edx
0xaf7ed: mov  edx,[ebp-0x1f4]    ; arg2
0xaf7f3: cmp  esi,edx
0xaf7f5: mov  eax,edx
0xaf7f7: cmovl eax,esi           ; eax = min(arg1,arg2)
0xaf7fa: add  ecx,eax            ; low + remainder
0xaf7fc: mov  [ebp-0x200],ecx    ; result
0xaf802: jmp  common-store
```

Notably there is **no shift by 15** anywhere near this site, and no
floating-point instruction; the reduction is a single unsigned `div`.

### 2.2 Cross-version check

The generic 3.5 runtime contains the same reduction code shape
(`work/w5a_ref/Game.exe`, command-dispatch function at RVA 0x60b6f0): its
Park-Miller routine is at RVA 0x53b2e0 (multiplier immediate `0x41A7` at
0x53b347), and the random case performs `call` + unsigned `div` + `min` + `add`
at RVAs 0x60f92a and 0x60feac:

```text
call 0x53b2e0            ; Park-Miller draw
xor  edx,edx
div  esi                 ; esi = abs(span)+1
... cmovl / add edx,eax  ; min + remainder
```

## 3. What the draw actually is

The retail handler does **not** call the MSVC LCG at RVA 0x2c3cd2 that
`WolfRng::next_raw` replays. It calls RVA `0x35810`:

* Park-Miller "minimal standard" LCG: multiplier `16807` (`0x41A7`), modulus
  `0x7fffffff`.
* Self-seeds from a clock import on first use (no Game.dat seed).
* Rejection loop: if the state exceeds `0x77359400` (2,000,000,000), multiply
  again until it does not; if it becomes 0, reseed from the clock.
* Returns a value in roughly `1..=2_000_000_000`.

So the retail `rand(min,max)` operates on a ~31-bit value, while M7 applied the
(then-inferred) multiply-high law to a 15-bit value. For a 40001-wide range the
old code could only reach `-20000 + draw` (max +12767); the landed event stream
spans both tails.

The MSVC LCG is real and is used elsewhere: the Game.dat static-random block
and data-file decryption. Re-deriving the packed Game.dat confirms the
`_wolfref/game_dat_random.c` generator exactly:

* Recovered stream state before the first table byte: `0x4605a618`
  (original seed state `0xe765b62e`); every table byte equals
  `(state >> 24) & 0x7f` of the next LCG state.
* `random_a = 22227` is **not** the seed: it is
  `(draw(S1) % 3000) + 20000 + 328` with `draw(S1) = 28899`, i.e. a draw two
  steps before the first table byte (`28899 % 3000 = 1899`).
* `random_b = 25684` likewise encodes `draw(S2) = 17925`
  (`17925 % 3000 = 2925`): `2925 + 200 + 4 + 328 + 22227 = 25684`.
* Therefore M7's `from_game` premise "random_a seeds the stream" is a
  reconstruction convenience, not the retail relationship. It does not affect
  the 0x6000 operator (which does not use this stream at all) but should be
  corrected before anyone claims byte parity for the static table.

## 4. Landed implementation

`crates/wolf-vm/src/rng.rs` now carries **two** streams:

1. **Event stream (0x6000).** `event_state: u32` advanced as
   `state = (state as u64 * 16807) % 0x7FFFFFFF`, with the retail rejection loop
   (`while state > 2_000_000_000: multiply again`) and a zero-state reseed.
   `next_event_draw() -> u32` returns the state. Seeds:

   * `EVENT_SEED = 1` — the library default, the canonical MINSTD anchor. The
     retail generator has no seed constant (it reads the clock), so this keeps
     tests and diagnostics deterministic; interactive `wolf-cli play` seeds
     from the clock instead.
   * `EVENT_RESEED = 1` — what a zero state becomes. State 0 is an LCG fixed
     point, so it is never kept; this is the chosen stand-in for the retail
     zero-state clock reread. Production wiring lives in `wolf-cli play` via
     `resolve_session_event_seed` (2026-11-03).
   * `from_event_seed(seed)` / `set_event_seed(seed)` reseed only this stream;
     `from_seeds(msvc, event)` seeds both. `new()` = MSVC `DEFAULT_SEED =
     22227` + event `EVENT_SEED = 1`.
   * `from_seed(seed)` keeps its M7 meaning (static-table stream, event stays
     at the default) and `from_game` is unchanged for API stability.

2. **Static-table stream (MSVC).** `state`, `next_raw()` and `state()` are
   unchanged: `state*0x343FD + 0x269EC3 (mod 2^32)`, draw
   `(state >> 16) & 0x7fff`. This stream is no longer consumed by `range()`.

`range(min, max)` consumes **one** event draw per call (including zero width)
and applies the pinned law:

```rust
let draw = self.next_event_draw();
let low = min.min(max);
let diff = max.wrapping_sub(min);
let modulus = diff.unsigned_abs().wrapping_add(1);
low.wrapping_add((draw % modulus) as i32)
```

The 32-bit wrapping keeps the retail edge case: `i32::MIN..=i32::MAX` has
`diff = -1`, `modulus = 2`, so only the lower two values are reachable.

## 5. Vectors and tests

MINSTD with seed 1 (pinned in `crates/wolf-vm/src/rng.rs` and
`crates/wolf-vm/tests/rng_retail.rs`):

```text
draw: 16807, 282475249, 1622650073, 984943658, 1144108930,
      470211272, 101027544, 1457850878, ...
```

`range()` under the modulo law, default event seed 1:

```text
range(1,10)          first 8: 8, 10, 4, 9, 1, 3, 5, 9
range(0,10)          first 8: 10, 1, 0, 6, 8, 3, 2, 0
range(-20000,20000)  first 8: -3193, 8188, -10492, 19036,
                              -19672, 19518, 5019, -5567
```

With event seed 22227 (for cross-checking the old seed choice):

```text
range(-20000,20000)  first 8: 19851, 2277, 19785, -13511,
                              19102, 13181, -6096, 8956
```

Tests now prove: inclusive bounds; both endpoints of `0..=1`; both tails of a
40001-wide range; swapped bounds draw-for-draw; a zero-width range consumes
exactly one draw; wide/extreme ranges; the rejection loop multiplies again
above 2e9; a zero seed reseeds; the two streams are independent; and the range
result matches an independent re-implementation of the decompiled law.

## 6. Verification and residual

```text
cargo test -q -p wolf-vm
```

Result (exit 0):

```text
lib:         91 passed, 0 failed
rng_retail:  15 passed, 0 failed
doctests:     0 passed, 0 failed
```

Sequence-sensitive game suites (all green after the repairs described in the
task report):

```text
WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' \
  cargo test -q -p wolf-game --test m6_mainline --test m6_training_menu \
  --test m6_night_menu --test m6_ev1_page0 --test m6_ev1_page2 \
  --test m6_multiday --test m6_night_sleep --test m6_week_loop \
  --test time_system -- --nocapture
```

Remaining work (exact parity):

1. **Clock seed — production wiring closed (2026-11-03).** Interactive
   `wolf-cli play` seeds from the local clock via
   `resolve_session_event_seed`; `WOLF_EVENT_SEED` injects a captured retail
   tick. The library keeps seed 1 so tests/diagnostics replay deterministically.
   The generator/range law needed no change.
2. **Same-save replay parity.** Same save + same event seed now replays exactly.
   To compare bit-for-bit with one particular retail run, capture that run's raw
   GetTickCount (RVA 0x35847) and pass it as `WOLF_EVENT_SEED`; without it the
   clocks differ by construction. Zero-tick behaviour is still assumed.
3. **Static-table story.** The MSVC stream's actual seed state for this Game.dat
   is recoverable (`0xe765b62e`); `random_a`/`random_b` are encoded draws, not
   seeds. `from_game` still accepts them for API stability but should not be
   described as "seeding the stream".