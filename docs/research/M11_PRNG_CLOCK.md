# M11-P: retail clock-seed provenance for the event PRNG

Status: implemented and tested. The Park-Miller event generator at RVA 0x35810
takes its first seed from the raw 32-bit return value of KERNEL32!GetTickCount.
There is no folding arithmetic: the DWORD is stored directly into the state at
VA 0x011b17f0 (RVA 0x00db17f0) before the first Park-Miller step.

This document records the disassembly evidence, the exact seed and draw
formulas, the test vectors, and the branches that cannot be proven without a
live retail process.

## 1. Artifact and address map

- Binary: work/runcopy/Game.exe
- SHA-256: 98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea
- Size: 7,329,792 bytes
- PE32, Machine 0x14c (i386), ImageBase 0x400000, entry RVA 0x000e8fe5
- Section used for code: .text, VA 0x00001000, Raw 0x000400, RawSize 0x401000
- Code file offset formula: file offset = 0x400 + (RVA - 0x1000)
- Function under study: RVA 0x00035810, file offset 0x00034c10

Instruction addresses below are RVAs (image-relative). Absolute memory operands
such as [0x011b17f0] are VAs; subtract the 0x400000 image base to get an RVA.
Data references used here:

- state: VA 0x011b17f0, RVA 0x00db17f0. It lies in the uninitialized virtual tail
  of .data, so it is zero-filled at process start. The .data section is VA
  0x004ae000 with VirtualSize 0x0090380c and RawSize 0x00203800; RVA 0x00db17f0
  is inside the virtual range but beyond the raw bytes, hence BSS zero.
- once-guard word: VA 0x011b15e8, RVA 0x00db15e8. Also in that BSS tail, so
  initially zero.

## 2. Calling convention and function shape

The function is __fastcall: mode arrives in ECX and the optional argument in
EDX. At entry, 0x3581d saves EDX to ESI and 0x35822 saves ECX to EDI. It returns
in EAX and uses a plain ret at 0x358e1 and 0x358e9; there is no arg cleanup.

Exact entry bytes:

    0x035810  55                       push ebp
    0x035811  8b ec                    mov  ebp, esp
    0x035813  83 e4 f8                 and  esp, 0xfffffff8
    0x035816  64 a1 2c 00 00 00        mov  eax, dword ptr fs:[0x2c]
    0x03581c  56                       push esi
    0x03581d  8b f2                    mov  esi, edx
    0x03581f  57                       push edi
    0x035820  8b 10                    mov  edx, dword ptr [eax]
    0x035822  8b f9                    mov  edi, ecx

## 3. Lazy first-use seeding: GetTickCount

The seeding block is guarded so it runs when the once-guard word is still the
lock-helper sentinel -1:

    0x035824  a1 e8 15 1b 01           mov  eax, dword ptr [0x011b15e8]
    0x035829  3b 82 04 00 00 00        cmp  eax, dword ptr [edx + 4]
    0x03582f  7e 2e                    jle  0x03585f
    0x035831  68 e8 15 1b 01           push 0x011b15e8
    0x035836  e8 1c 30 0b 00           call 0x000e8857        ; _lock on guard word
    0x03583b  83 c4 04                 add  esp, 4
    0x03583e  83 3d e8 15 1b 01 ff     cmp  dword ptr [0x011b15e8], -1
    0x035845  75 18                    jne  0x03585f
    0x035847  ff 15 44 20 80 00        call dword ptr [0x00802044] ; GetTickCount
    0x03584d  68 e8 15 1b 01           push 0x011b15e8
    0x035852  a3 f0 17 1b 01           mov  dword ptr [0x011b17f0], eax ; state = tick
    0x035857  e8 b1 2f 0b 00           call 0x000e880d        ; _unlock
    0x03585c  83 c4 04                 add  esp, 4

The once behavior follows from the helper pair. The BSS guard starts at zero.
The lock helper at RVA 0x000e8857 changes a zero guard to -1; the RNG then sees
cmp guard,-1 equal and performs the seed. The unlock helper at RVA 0x000e880d
increments a counter and writes the incremented (positive) value back through
the guard pointer:

    RVA 0x0e8823  mov eax, [ebp + 8]       ; pointer to the guard
    RVA 0x0e8826  inc ecx                  ; counter++ 
    RVA 0x0e882d  push esi
    RVA 0x0e882e  mov [eax], ecx           ; guard = positive counter
    ...
    RVA 0x0e8852  jmp 0x000e88a9           ; unlock/free path

On later entries the guard is positive, cmp guard,-1 is false, and the
GetTickCount block is skipped. This makes the clock seed a one-time lazy seed on
first use, not a per-call reseed.

Import-table proof that the called import is GetTickCount:

- IAT slot 0x00802044 resolves to KERNEL32.dll!GetTickCount.
- Over the 0xda bytes of this function, the absolute operand 0x00802044 occurs
  exactly twice: RVA 0x35847 (the initial seed) and RVA 0x358d1 (the zero-state
  fallback described below).
- QueryPerformanceCounter IAT 0x008021b4 occurs 0 times in the function.
- GetSystemTimeAsFileTime IAT 0x008021bc occurs 0 times.
- GetTimeZoneInformation IAT 0x00802104 occurs 0 times.

Therefore the clock source is specifically GetTickCount (milliseconds since
system start), not QueryPerformanceCounter, FILETIME, time(), or timezone state.

## 4. Mode dispatch: advance, peek, or explicit set

After the one-time seed block, the function dispatches on ECX:

    0x03585f  83 ef 00                 sub edi, 0
    0x035862  74 0d                    je  0x035871          ; mode 0: advance
    0x035864  83 ef 02                 sub edi, 2
    0x035867  74 5e                    je  0x0358c7          ; mode 2: set state
    0x035869  8b 35 f0 17 1b 01        mov esi, [0x011b17f0] ; other: peek
    0x03586f  eb 5c                    jmp 0x0358cd

- mode 0: advance the Park-Miller stream and return the new state.
- mode 2: store EDX (saved in ESI) into the state and go to the common zero
  check; this is an explicit external reseed.
- other: load the current state and return it without advancing (peek).

The opcode-121 random operator calls mode 0. See section 6.

## 5. Exact mode-0 Park-Miller step and zero fallback

    0x035871  a1 f0 17 1b 01           mov  eax, [0x011b17f0]
    0x035876  bf a7 41 00 00           mov  edi, 0x41a7          ; 16807
    0x03587b  f7 e7                    mul  edi
    0x03587d  6a 00                    push 0
    0x03587f  68 ff ff ff 7f           push 0x7fffffff
    0x035884  8b f0                    mov  esi, eax
    0x035886  52                       push edx
    0x035887  56                       push esi
    0x035888  e8 03 02 2b 00           call 0x002e5a90        ; unsigned 64/64 divide
    0x03588d  69 c0 ff ff ff 7f        imul eax, eax, 0x7fffffff
    0x035893  2b f0                    sub  esi, eax            ; state = product % modulus
    0x035895  89 35 f0 17 1b 01        mov  [0x011b17f0], esi
    0x03589b  81 fe 00 94 35 77        cmp  esi, 0x77359400      ; 2,000,000,000
    0x0358a1  76 2a                    jbe  0x0358cd
    0x0358a3  8b c6                    mov  eax, esi
    0x0358a5  f7 e7                    mul  edi
    ... rejection loop repeats the same multiply/divide/remainder until <= 2e9 ...
    0x0358c5  77 dc                    ja  0x0358a3

So the mode-0 recurrence is exactly:

    state = state * 16807 % 0x7fffffff
    while state > 2_000_000_000:
        state = state * 16807 % 0x7fffffff
    return state

The common zero check is after both mode 0 and mode 2:

    0x0358c7  89 35 f0 17 1b 01        mov  [0x011b17f0], esi   ; mode 2 store
    0x0358cd  85 f6                    test esi, esi
    0x0358cf  75 11                    jne 0x0358e2
    0x0358d1  ff 15 44 20 80 00        call dword ptr [0x00802044] ; GetTickCount again
    0x0358d7  a3 f0 17 1b 01           mov  [0x011b17f0], eax
    0x0358dc  5f                       pop edi
    0x0358dd  5e                       pop esi
    0x0358de  8b e5                    mov esp, ebp
    0x0358e0  5d                       pop ebp
    0x0358e1  c3                       ret                      ; returns raw tick in EAX
    0x0358e2  5f                       pop edi
    0x0358e3  8b c6                    mov eax, esi
    0x0358e5  5e                       pop esi
    0x0358e6  8b e5                    mov esp, ebp
    0x0358e8  5d                       pop ebp
    0x0358e9  c3                       ret                      ; returns state in EAX

If the resulting state is zero, retail re-reads GetTickCount, stores that raw
value as the state, and returns the raw value. This differs from the ordinary
draw path: the returned value is not passed through a Park-Miller multiply and
is not range-limited by the 2e9 ceiling. A mode-2 call with EDX=0 enters this
same fallback.

## 6. Caller evidence

Opcode 121's random case is the consumer that matters for the event stream. The
selected block is at RVA 0x0af7c0:

    0x0af7c0  33 d2                    xor edx, edx
    0x0af7c2  33 c9                    xor ecx, ecx
    0x0af7c4  e8 47 60 f8 ff           call 0x0035810        ; mode 0, advance
    0x0af7c9  8b f0                    mov esi, eax
    0x0af7cb  8b 85 10 fe ff ff        mov eax, [ebp - 0x1f0]
    0x0af7d1  2b 85 0c fe ff ff        sub eax, [ebp - 0x1f4]
    0x0af7d7  99                       cdq
    0x0af7d8  8b c8                    mov ecx, eax
    0x0af7da  8b c6                    mov eax, esi
    0x0af7dc  33 ca                    xor ecx, edx
    0x0af7de  8b b5 10 fe ff ff        mov esi, [ebp - 0x1f0]
    0x0af7e4  2b ca                    sub ecx, edx
    0x0af7e6  33 d2                    xor edx, edx
    0x0af7e8  41                       inc ecx
    0x0af7e9  f7 f1                    div ecx
    0x0af7eb  8b ca                    mov ecx, edx
    0x0af7ed  8b 95 0c fe ff ff        mov edx, [ebp - 0x1f4]
    0x0af7f3  3b f2                    cmp esi, edx
    0x0af7f5  8b c2                    mov eax, edx
    0x0af7f7  0f 4c c6                 cmovl eax, esi
    0x0af7fa  03 c8                    add ecx, eax
    0x0af7fc  89 8d 00 fe ff ff        mov [ebp - 0x200], ecx

This both confirms the mode-0 consumer and independently re-confirms the range
law already documented in M8_PRNG_RANGE.md: low=min(a,b), modulus=abs(b-a)+1,
value=low+draw%modulus.

An explicit mode-2 setter wrapper also exists at RVA 0x09aa77:

    0x09aa77  8b 55 0c                 mov edx, [ebp + 0xc]
    0x09aa7a  b9 02 00 00 00           mov ecx, 2
    0x09aa7f  e8 8c ad f9 ff           call 0x0035810        ; state = arg
    0x09aa84  5f                       pop edi
    0x09aa85  5e                       pop esi
    0x09aa86  5b                       pop ebx
    0x09aa87  8b e5                    mov esp, ebp
    0x09aa89  5d                       pop ebp
    0x09aa8a  c2 08 00                 ret 8

This shows the binary has a later gameplay path that can overwrite the stream
with an explicit value. That overwrite does not change the provenance of the
initial/once seed, which is still GetTickCount. Which gameplay event invokes
this wrapper, and whether it can run before the first draw, is not established
by this slice.

## 7. Proven seed formula and implementation

Retail initial seed:

    seed0 = GetTickCount() as u32          ; raw milliseconds, no folding
    state = seed0

First mode-0 draw:

    state = state * 16807 % 0x7fffffff
    while state > 2_000_000_000:
        state = state * 16807 % 0x7fffffff
    return state

No add, xor, shift, multiply, or hash is applied to the GetTickCount result
before it is stored. The folding function is the identity:

    seed_from_clock_ticks(ticks) = ticks

Added to crates/wolf-vm/src/rng.rs:

- pub const fn seed_from_clock_ticks(ticks: u32) -> u32 { ticks }
- pub fn WolfRng::from_clock_ticks(ticks: u32) -> Self
  which calls from_event_seed(seed_from_clock_ticks(ticks)).

Existing API and defaults are unchanged: WolfRng::new() still uses
DEFAULT_SEED=22227 for the MSVC static stream and EVENT_SEED=1 for the
Park-Miller event stream, and no clock is read by default. The new constructor
only injects a caller-supplied captured tick.

A zero tick is a deliberate divergence from retail because its fallback cannot
be replayed from one captured tick: retail re-reads GetTickCount at RVA 0x358d1.
This build normalises zero through from_event_seed to EVENT_RESEED=1. That is
documented and pinned by a test. Because EVENT_RESEED is 1, a zero tick produces
the same stream as a tick of 1; the tests assert both this concrete substitution
and divergence from a different tick.

## 8. Tests and observed output

New integration test: crates/wolf-vm/tests/prng_clock_seed.rs

    cargo test -q -p wolf-vm --test prng_clock_seed
    running 5 tests
    .....
    test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

The five tests cover:

1. clock_ticks_are_stored_without_folding: identity for 0, 1, 1000,
   0x12345678 and u32::MAX, plus event_seed() equality.
2. retail_tick_stream_matches_independently_computed_vectors: first eight
   draws for seven ticks against Python-generated vectors and against an
   independent in-test Park-Miller implementation.
3. same_tick_replays_identically_and_different_ticks_diverge: 64 draws replay
   for one tick, differ for tick+1, plus range(1,100) vectors.
4. zero_tick_uses_documented_deterministic_fallback: zero maps to EVENT_RESEED,
   equals the tick-1 stream, and differs from tick 1000.
5. clock_seed_api_does_not_change_existing_defaults: new() and
   from_event_seed(EVENT_SEED) still return the pinned seed-1 stream.

Vectors used (first eight Park-Miller draws; seed == tick):

    tick 1:          16807, 282475249, 1622650073, 984943658, 1144108930, 470211272, 101027544, 1457850878
    tick 1000:       16807000, 1154891243, 1299919515, 1396147674, 1647629796, 95812591, 1856965334, 636526687
    tick 1000000:    1774614471, 1692524561, 691908565, 283303450, 509838751, 400136527, 1323310532, 1539462992
    tick 3600000:    375657884, 80134208, 343387187, 1019892420, 117432586, 152001309, 1327944080, 131021851
    tick 0x12345678: 706275742, 1234278825, 1979665402, 1272268443, 521048322, 1968319035, 1699922857, 481017911
    tick 123456789:  469049721, 1781357515, 1206231778, 891865166, 141988902, 553144097, 236130416, 94122056
    tick 0x7ffffffe: 1865008398, 524833574, 1162539989, 1003374717, 1677272375, 689632769, 688705724, 140245938

Range(1,100) first eight from tick 1000000: 72, 62, 66, 51, 52, 28, 33, 93.
From tick 1000001: 79, 11, 92, 9, 82, 100, 77, 24.

## 9. Residual unknowns (not provable from this static analysis)

- The live value of GetTickCount at the first draw depends on process start
  time and machine uptime; only the source and the identity folding are proven.
- The zero-state branch calls GetTickCount again at RVA 0x358d1. Reproducing
  retail exactly from a single captured tick is impossible when that branch is
  taken; the build substitutes EVENT_RESEED and documents it.
- GetTickCount is a 32-bit millisecond counter and wraps roughly every 49.7
  days. The disassembly proves the raw DWORD is used; it does not prove how a
  live process behaves across the wrap boundary.
- The explicit mode-2 setter wrapper at RVA 0x09aa77 proves the stream can be
  overwritten later, but this slice does not map which gameplay events call it
  or whether any can occur before the first event draw.
- The once-guard helper bodies were inspected at RVA 0x000e8857 and
  0x000e880d; their higher-level CRT lock semantics are inferred from the
  writes to the guard word, not from symbol names. The invariant used here is
  narrow and directly observable: first entry sees zero, lock sets -1, seed
  runs; unlock writes a positive counter, later entries skip the seed.
- No live retail process was run under a debugger, and there is no PDB, so
  runtime ordering (for example another thread drawing before this function)
  is not proven.
