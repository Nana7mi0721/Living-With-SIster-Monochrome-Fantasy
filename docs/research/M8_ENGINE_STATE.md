# M8 W3-b — engine-state fields 11 / 12 / 17 pinned from the shipped binary

Status: **fields 11/12 pinned and implemented; field 17 pinned but not answerable
by the Host trait, so it keeps the documented neutral 0.** No commit/push.
Date: 2026-10-31. Base: `origin/master` `7c28480` (worktree
`.claude/worktrees/agent-a563ff119f2d83aee`).

Binary: `work/runcopy/Game.exe`, PE32, image base 0x400000, sha256
`98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea`
(7,329,792 bytes — the same file used by `M8_PRNG_RANGE.md`). All addresses
below are **RVAs**; file offset = `0x400 + (rva - 0x1000)` for the code region.

## 1. Verdict (read this first)

`docs/OPCODE_124.md` §2 followed the official editor field table and therefore
labelled fields 11/12 as "map event id under the mouse" / "contact event id".
The shipped handler does **not** compute anything for 11/12:

| field | retail answer | instruction |
|---|---|---|
| 10 | wheel rotation delta | `mov edx,[0x11a9100]` |
| **11** | **mouse X travel since the previous frame** (signed) | `mov edx,[0x11a9104]` |
| **12** | **mouse Y travel since the previous frame** (signed) | `mov edx,[0x11a9108]` |
| 13 | map event id under the mouse pointer (minimum id, -1 if none) | a loop over the map event list (0xb367e–0xb3811) |
| **17** | **this common event id** (`[event+0x18]`; -1 for a map event) | `mov edx,[edi+0x18]` |

The two globals are maintained by the per-frame input update, which stores
`cursor - previous_cursor`; they are not event ids and never use -1 as a
"nothing" sentinel. `-1` was the wrong answer for both fields.

Field 17 is *not* an event-line counter: the line/PC of the running event is
field 19 (`[running+8]`). The new official index 17 = 処理中のEv行数 does not
apply to this build (its indices are shifted, just as BGM is 1 here and 8 in
the new docs).

## 2. Evidence

### 2.1 Dispatch chain

The command dispatcher at RVA 0xabe10 selects handlers through a byte table at
`0x4ad28c` and a jump table at `0x4ad1bc`:

```text
0x4abebc: movzx eax, byte ptr [ecx + 0x4ad28c]   ; ecx = opcode + 1
0x4abec3: jmp   dword ptr [eax*4 + 0x4ad1bc]
```

For opcode `0x7c` (124) the table entry is index 13 → RVA `0xabf04`:

```text
0x4abef8: mov ecx, ebx
0x4abefa: call 0x4aea70        ; opcode 121 (SetVariable)
0x4abf04: mov ecx, ebx
0x4abf06: call 0x4b25b0        ; opcode 124 (SetVariableEx)
```

Handler RVA **0xb25b0**. It evaluates the sub-type and switches:

```text
0x4b269b: sar  eax, 0xc
0x4b26a0: and  eax, 0xf
0x4b26a3: dec  eax
0x4b26a4: cmp  eax, 3
0x4b26a7: ja   default
0x4b26ad: jmp  dword ptr [eax*4 + 0x4b3bc4]
```

Sub-type switch table (VA 0x4b3bc4, four entries):

| index | type nibble | target RVA | meaning |
|---|---|---|---|
| 0 | 1 | 0xb26b4 | `0x1000` / `0x1200` character |
| 1 | 2 | 0xb2b29 | `0x2000` (no sites in this game) |
| 2 | 3 | **0xb3497** | `0x3000` other |
| 3 | 4 | 0xb345c | `0x4000` / `0x4400` picture |

The `0x3000` branch reads the field operand and switches over 0..26:

```text
0x4b3497: push 3                       ; arg[2] (argc 3 = [dst, sub, field])
0x4b3499: mov  ecx, esi
0x4b349b: call 0x4c8e40
0x4b34a0: mov  eax, [eax]
0x4b34a2: cmp  eax, 0x1a
0x4b34a5: ja   0x4b271d
0x4b34ab: jmp  dword ptr [eax*4 + 0x4b3cb4]
```

`0x3000` field switch table (VA 0x4b3cb4, 27 entries):

| field | target RVA | field | target RVA |
|---|---|---|---|
| 0 | 0xb34b2 | 14 | 0xb3816 |
| 1 | 0xb34cb | 15 | 0xb381e |
| 2 | 0xb34e1 | 16 | 0xb382f |
| 3 | 0xb34f7 | **17** | **0xb3840** |
| 4 | 0xb353d | 18 | 0xb3848 |
| 5 | 0xb356a | 19 | 0xb389f |
| 6 | 0xb35ca | 20 | 0xb38fc |
| 7 | 0xb35f7 | 21 | 0xb3907 |
| 8 | 0xb3619 | 22 | 0xb3912 |
| 9 | 0xb363b | 23 | 0xb395d |
| 10 | 0xb365d | 24 | 0xb399c |
| **11** | **0xb3668** | 25 | 0xb39b8 |
| **12** | **0xb3673** | 26 | 0xb39c3 |
| 13 | 0xb367e | | |

### 2.2 Fields 11 / 12 are mouse travel

The switch bodies are one instruction each:

```text
0xb365d: 8b 15 00 91 1a 01   mov edx, dword ptr [0x11a9100]   ; field 10
0xb3668: 8b 15 04 91 1a 01   mov edx, dword ptr [0x11a9104]   ; field 11
0xb3673: 8b 15 08 91 1a 01   mov edx, dword ptr [0x11a9108]   ; field 12
```

Those addresses belong to the input singleton based at `0x11a90c0`:

* getter RVA 0x483cc0: `call 0x483cd0; mov eax, 0x11a90c0; ret`;
* the constructor at 0x483cd0 zeroes `0x11a9100`, `0x11a9104`, `0x11a9108`
  (and `0x11a9150`/`0x11a9154`, the previous-position pair) at startup;
* the per-frame input update `0x4846d0` receives `this = 0x11a90c0`.

That update clamps the cursor, optionally divides by the screen scale, then
subtracts the previous stored position **of the same input update's previous
run**, i.e. the previous frame:

```text
0x484e53: mov esi, [ebx + 0x20]     ; cursor X (field 20 later reads this)
0x484e56: mov eax, [0xab0880]       ; max X
0x484e5b: cmp esi, eax
0x484e5d: jle 0x484e64
0x484e5f: mov [ebx + 0x20], eax
0x484e64: test esi, esi
0x484e66: jns 0x484e71
0x484e68: mov dword ptr [ebx + 0x20], 0   ; clamp negative to 0
0x484e6f: xor esi, esi
...
0x484eae: 8b c6 2b 83 90 00 00 00   mov eax, esi; sub eax, [ebx + 0x90]
0x484eb6: 89 43 44                  mov [ebx + 0x44], eax    ; field 11
0x484eb9: mov eax, ecx
0x484ebb: sub eax, [ebx + 0x94]                              ; cursor Y - prev Y
0x484ec1: cmp byte ptr [esp + 0x19], 0
0x484ec6: mov [ebx + 0x48], eax                              ; field 12
0x484ec9: 89 b3 90 00 00 00         mov [ebx + 0x90], esi    ; remember X
0x484ecf: mov [ebx + 0x94], ecx                              ; remember Y
```

So both fields are **signed differences**, `0` when the pointer stands still,
negative when it moves left/up. (`[ebx+0x40]`, field 10, is the wheel delta,
written at 0x484dc3/0x484ddf/0x484df4 from the wheel API 0x525b20.)

### 2.3 Field 13 is the "event under the pointer" query

0xb367e–0xb3811 iterates the map event list at `0x11a8428` (count via
`0x4c6b10`, element via `0x4c6b00`). It maps the cursor into the map's pixel
space using the map singleton `0x11a6618` (width accessor 0x4a5dc0) and the
tile size `[0xab0870]`, compares against the event's pixel position
(`[event+0x34]` x, `[event+0x38]` y) widened by the half-tile flag
`[event+0xad]`, and keeps

```text
0x4b37e2: cmp edx, dword ptr [eax + 0x20]   ; current best vs event id
0x4b37e5: jl  0x4b3804                      ; keep the SMALLER id
0x4b37ed: mov edx, dword ptr [eax + 0x20]
```

returning `-1` when nothing matches (`0x4b39df`). That is the documented
"マウス座標下のマップイベントID … 複数ある場合は最小" semantics, at **field
13**, not 11/12. This game has zero field-13 sites (see §3).

### 2.4 Field 17 is this common event id

```text
0xb3840: 8b 57 18   mov edx, dword ptr [edi + 0x18]
```

`edi` is the event interpreter instance (`ecx` at handler entry, saved at
`[ebp-0x284]`; the `0x3000` branch never replaces it). The instance is built by
the two-stage constructor at RVA 0x4a9b00, after the base initialiser 0x4a9a60
sets both slots to -1:

```text
0x4a9b06: call 0x4a9a60
0x4a9b0b: mov eax, [0x11a80c4]      ; current map id
0x4a9b10: mov [esi + 0x10], eax
0x4a9b13: 8b 45 0c 89 46 14         mov eax,[ebp+0xc]; mov [esi+0x14],eax  ; arg2
0x4a9b19: 8b 45 10 89 46 18         mov eax,[ebp+0x10]; mov [esi+0x18],eax ; arg3
0x4a9b1f: mov eax, [ebp + 8]        ; program
0x4a9b27: mov [esi + 0x808], eax
```

The start sites fix the meaning of the two slots (cdecl: last push = arg1):

* **map event autorun/parallel** (0x4aa70b–0x4aa71f) pushes
  `[-1, event_id, program]` → `+0x14 = event id`, `+0x18 = -1`;
* **pending common event** (0x4aa614–0x4aa620) pushes
  `[common_event_id, caller[+0x14], program]` → `+0x14 = caller's trigger id`,
  `+0x18 = the common event number being started`;
* another common-event start (0x4aa745–0x4aa755) pushes `[[eax], -1, program]`
  → `+0x14 = -1`, `+0x18 = [eax]` (the scheduled common event id).

Opcode 210's relative common-event call confirms the slot is the **current
common event id**: 0x4b72b4 does

```text
mov eax, [esi + 0x18]      ; current common event number
add eax, 0xfff6d7dc        ; -600100
add eax, ebx               ; encoded relative target
... clamp to [0, common_event_count-1]
add ebx, 0x7a120           ; +500000
mov [0x11a6234], ebx       ; pending event number
```

Field 14 (`0xb3816: mov edx,[edi+0x14]`) is the other slot: the triggering
event id (the map event that ultimately started the chain; its own id for a
self-started map event). Field 17 is therefore best named **このコモンイベントID**
and answers -1 for a map-event instance.

### 2.5 Neighbouring fields (for completeness)

| field | RVA | instruction / callee | retail meaning |
|---|---|---|---|
| 0 | 0xb34b2 | `[0x11a80c4]` / `[0x11a6244]` | current map id |
| 1 | 0xb34cb | `[0x11aa268]`, -1 if >= `[0x11a64e4]` | playing BGM id |
| 2 | 0xb34e1 | `[0x11aa26c]` | playing BGS id |
| 3–6 | 0xb34f7–0xb35ca | audio position/length calls | BGM/BGS position and length |
| 7/8/9 | 0xb35f7/0xb3619/0xb363b | `[0x11a90dd]`/`[0x11a90de]`/`[0x11a90df]` vs `[0x11a90da]`/… | left/right/middle tri-state (2 = just pressed, 1 = held, 0 = up) |
| 10 | 0xb365d | `[0x11a9100]` | wheel rotation delta |
| 11 | 0xb3668 | `[0x11a9104]` | **mouse X travel** |
| 12 | 0xb3673 | `[0x11a9108]` | **mouse Y travel** |
| 13 | 0xb367e | event-list loop | **map event under the cursor** (min id) |
| 14 | 0xb3816 | `[edi+0x14]` | 起動EvID (triggering event id) |
| 15 | 0xb381e | `call 0x4a5dc0` → `[map+0x1a7c]` | map width (tiles) |
| 16 | 0xb382f | `call 0x4a5dd0` → `[map+0x1a80]` | map height (tiles) |
| 17 | 0xb3840 | `[edi+0x18]` | **this common event id** (-1 = map event) |
| 18 | 0xb3848 | running-event list 0x11a99e4 | running event number; `[node+0x18] + 500000` when common, else `[node+0x14]`, -1 when none |
| 19 | 0xb389f | `[running+8]` | current line / PC of the running event |
| 20 | 0xb38fc | `[0x11a90e0]` | cursor X |
| 21 | 0xb3907 | `[0x11a90e4]` | cursor Y |
| 22 | 0xb3912 | process-memory API | consumed memory |
| 23 | 0xb395d | time API (`shrd … 0x14`) | time |
| 24 | 0xb399c | `byte [0x11a85bc] / 100` lookup | language |
| 25 | 0xb39b8 | `[0x11a8090]` | pad type |
| 26 | 0xb39c3 | `call 0x4c6b10` on 0x11a8428 | map event count |

Field counts in this game's scripts (CE + 9 maps, 0x3000, argc 3) are unchanged
from `M7_OPCODE_124.md`: `{0:3, 1:1, 7:121, 8:21, 9:1, 10:10, 11:1, 12:1,
17:5, 18:3, 20:4, 21:3}`. The only fields 11/12 sites are CE225 pc70/71 and all
five field-17 sites are CE265/266/267/268/270.

## 3. What the port now answers

`GameHost::engine_state` (`crates/wolf-game/src/game.rs`):

| field | before | now | why |
|---|---|---|---|
| 0 | `0` | `0` | no current-map id is tracked; 0 is the boot map. Still a guess after a `130` switch. |
| 1 | `-1` | `-1` | the port records sound commands but not "which BGM data id is playing". |
| 7/8/9 | tri-state | unchanged | matches retail. |
| 10 | `0` | `0` | no wheel channel exists; "no rotation" is truthful, not a guess. |
| 11 | `-1` | **`cursor_x.max(0) - previous_cursor_x`** | pinned; real per-frame travel. |
| 12 | `-1` | **`cursor_y.max(0) - previous_cursor_y`** | pinned; real per-frame travel. |
| 13 | `None` | `None` | pinned meaning but zero game sites; the hit test needs per-event pixel geometry/`+0xad` not retained by `MapRuntime`. Not guessed. |
| 17 | `0` | `0` still | pinned meaning needs the interpreter's current instance; the `Host` trait has no such context (field 18 is intercepted inside `wolf-vm` for exactly this reason). All five sites use it only as a message-window guard, so the neutral 0 keeps their behaviour. |
| 18 | interpreter | unchanged | interpreter returns `inst.event + 500000` unconditionally; retail returns the raw map event id for map events and -1 when nothing runs. No map-event 18 site exists in this game. Outside this slice's write scope. |
| 20/21 | raw `self.mouse` | unchanged | retail returns the clamped, scale-divided cursor; the port's mouse is already logical and supplies `-1` off-window. Only off-window values diverge. |

`Game` gained one field, `previous_mouse: [i32; 2]`, initialised to `[0, 0]`
(the shipped input object is zero-initialised) and refreshed at the end of every
`Game::frame` with the same lower clamp the engine applies (`max(0)`). That is
the only state needed to answer 11/12 across frames. The upper clamp
(`0xab0880`) and the screen-scale division (`0xab087c`) are not modelled
because the port's cursor is already in logical pixels.

## 4. Tests

`crates/wolf-game/tests/m7_host_state.rs` (real `Game`, real opcode-124
programs; existing tests 2 and 3 unchanged in intent):

1. `engine_state_reports_the_mouse_and_neutral_none_sentinels_through_opcode_124`
   — one warm-up frame puts the pointer at (100, 100), then a frame at
   (640, 300) queries fields 20/21/17/11/12/7 through 124. Asserts
   `640, 300, 0, 540, 200, 2`; warnings 0. The three `-1`/`0` sentinels of the
   old test are gone because retail does not use them here.
2. `engine_travel_is_zero_without_movement_and_signed_when_moving_back` (new)
   — pointer unchanged ⇒ 11/12 = 0/0 (proves 0 is a computed value, not the
   poison value); pointer moved to (10, 20) ⇒ -630/-280 (proves signed).
3. `picture_state_field_five_answers_zero_for_live_pictures_only` — unchanged.
4. `character_state_stays_none_so_no_coordinates_are_fabricated` — unchanged
   (a `Game::new` runtime has no leader, so -2 still answers `None`).

## 5. Commands and results

```text
cargo test -q -p wolf-game --lib
# 230 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test m7_host_state
# 4 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' \
  cargo test -q -p wolf-game --test m6_mainline --test m6_multiday \
  --test m6_week_loop --test m6_night_sleep
# 7 + 1 + 1 + 1 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test m8_character_state --test m7_hmode_state \
  --test m7_hscene_playable --test m7_day4_playable
# 1 + 3 + 1 + 3 passed; 0 failed
```

## 6. Proven / still inferred

**Proven by decompilation (exact RVAs above):**

* fields 11/12 read the two globals at `0x11a9104` / `0x11a9108`, and the input
  update writes `cursor - previous_cursor` (signed) there each frame;
* field 13 is the map-event-under-cursor loop;
* field 17 reads the interpreter instance's `+0x18`, which every start path
  fills with the common event id (or -1 for map events), and opcode 210 uses it
  for relative common-event calls;
* field 14 is the trigger slot, field 19 the line/PC, fields 15/16 map size,
  field 26 event count, field 18 the +500000 running-event rule.

**Inferred / not proven:**

1. The retail cursor's upper clamp and screen-scale division are not modelled;
   the port keeps logical pixels (scale 1) and clamps only at 0. A windowed
   cursor larger than the screen would produce a larger travel value in the
   port than in retail.
2. Field 17's name ("this common event id") is inferred from the start-site
   arguments and the opcode-210 consumer; the manual name is not printed by the
   binary. The value for the five game sites (265/266/267/268 and 270) is
   therefore still not answered — the Host trait lacks interpreter context.
3. Field 0/1 remain honest boot guesses, and field 18's map-event form is
   answered by `wolf-vm` rather than the host; neither is changed here because
   they are outside this slice's write scope.