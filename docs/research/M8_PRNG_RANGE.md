# M8 W5-a: retail range reduction for 121 / operator 0x6000 `rand(min,max)`

Status: **PINNED by decompilation**; the range law itself is exact, but the code
change was **reverted** because the port still feeds it the wrong stream. Retail
draws 31-bit Park-Miller; `WolfRng::next_raw` draws 15-bit MSVC. Applying the
correct modulo law to the wrong stream changed player-visible sequences (the
day-menu roll, sister sprite pick) without increasing fidelity, so the port keeps
the old reduction until the Park-Miller generator lands in the same change.
§6 item 1 is the follow-up task. No commit/push.

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

## 3. What the draw actually is (new finding, not yet ported)

The retail handler does **not** call the MSVC LCG at RVA 0x2c3cd2 that
`WolfRng::next_raw` replays. It calls RVA `0x35810`:

* Park-Miller "minimal standard" LCG: multiplier `16807` (`0x41A7`), modulus
  `0x7fffffff`.
* Self-seeds from a clock import on first use (no Game.dat seed).
* Rejection loop: if the state exceeds `0x77359400` (2,000,000,000), multiply
  again until it does not; if it becomes 0, reseed from the clock.
* Returns a value in roughly `1..=2_000_000_000`.

So the retail `rand(min,max)` operates on a ~31-bit value, while the current
port applies the (now-correct) modulo law to a 15-bit value. Concrete
consequence for a 40001-wide range: retail spans both tails; the port currently
reaches only `-20000 + draw` (max 12767). This is documented in the tests, not
hidden.

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
  this task (the 0x6000 operator does not use this stream at all) but should be
  corrected before anyone claims byte parity for the static table.

## 4. Code changes (prototype reverted — see the status note)

The prototype below was tested and then reverted with
`git checkout -- crates/wolf-vm/src/rng.rs crates/wolf-vm/tests/rng_retail.rs`.
It is recorded because the exact law must be re-applied when the Park-Miller
stream lands. Reverting restored the 5 day-loop tests it broke.

`crates/wolf-vm/src/rng.rs` (prototype)

* `range()` now implements the pinned law:
  `diff = max.wrapping_sub(min)`, `modulus = diff.unsigned_abs().wrapping_add(1)`,
  `low = min.min(max)`, `low.wrapping_add((draw % modulus) as i32)`.
* It always calls `next_raw()` once per range call, including zero-width spans,
  matching the retail call order.
* 32-bit wrapping is deliberate: for `i32::MIN..=i32::MAX` retail's `max-min`
  wraps to `-1`, giving modulus 2, and the port reproduces that.
* Module docs now separate the two retail generators and mark the remaining
  generator gap.

`crates/wolf-vm/tests/rng_retail.rs` (+ `rng.rs` unit tests)

* New law test: `range` is compared draw-for-draw against
  `low + draw % (abs(max-min)+1)` for plain, reversed, zero-width, very wide
  and extreme ranges.
* New pinned vectors under the modulo law (default seed 22227):

```text
range(1,10)          first 8: 7, 1, 4, 6, 3, 9, 7, 7
range(-20000,20000)  first 8: -12914, -18940, -14447, -1585,
                              5972, 10038, -4434, -7774
```

  (multiply-high used to give `3, 1, 2, 6, 8, 10, 5, 4` for `range(1,10)`)
* New test: a zero-width range advances the stream exactly once, like retail.
* Updated the old `a_wide_range_uses_both_halves` test. It now documents the
  honest limitation: with a 15-bit draw and a 40001-wide range, only the lower
  part is reachable (3000 draws: min -19997, max 12748).

## 5. Verification

```text
cargo test -q -p wolf-vm
```

Result (exit 0):

```text
lib:            90 passed, 0 failed
rng_retail:     14 passed, 0 failed
other suites:   4 + 7 + 4 passed, 0 failed (1 ignored in one suite)
doctests:       0 passed, 0 failed
```

## 6. Remaining work (exact parity)

1. Port RVA 0x35810 into the VM's event RNG: Park-Miller 16807 / 2^31-1,
   rejection > 2,000,000,000, clock seed on first use. Then feed its 31-bit
   output into the pinned modulo law. This changes `WolfRng`'s draw domain and
   therefore battle/H-scene sequences; it should be its own task with a fresh
   same-save replay.
2. Decide how to model the clock seed for determinism: expose
   `from_seed`/`debug_seed` and keep the production path self-seeding, or seed
   from a save-time value. Exact retail vectors cannot exist without capturing
   a run's clock seed.
3. Correct the static-table story: the MSVC stream's actual seed state for
   this Game.dat is recoverable (0xe765b62e); `random_a`/`random_b` are encoded
   draws, not seeds.