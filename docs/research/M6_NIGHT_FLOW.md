# M6 Night Flow — natural route from 19:30 day2 bedroom → night menu CE245 → sleep?

Status: read-only investigation, HEAD a58505a. Deliverable: this file only.
Scope: can a player, at 19:30 on day2 in the bedroom, open the night command menu (CE245) and sleep to the next day **without injecting var63/var64**?

## TL;DR

- The **sleep primitive works**: starting CE245 at 19:30 day2 with `var69==1` (no var63/64 injection), clicking `今天先休息吧` (pic 1212 at 512,450) → choice 是/算了 → 是 drives CE327 → CE328 → CE329 and rolls **day2 19:30 → day3 08:30** (observed). var63 and var64 stay 0.
- The **bedroom→CE245 trigger exists**: hovering the sister sprite **pic120** (drawn at x=240,y=340, anchor=top-left) and pressing the **Decide key** sets `var68=1` (SampleMapA ev1 page0 pc11), which makes the autorun **SampleMapA ev4 page0** ("メニュー処理") run and fall through to **pc335 → CE245** (event 245 becomes active, observed at frame 8).
- **BUT** ev4 page0 is hard-gated by `var295==1` (pc1 → `var68=0; BreakEvent`). In the fast-forward-reached day2 state `var295==1`, so `var68` is immediately cleared and **nothing opens** — this is why prior "idle 6000 frames / click 107-110,121,122 does nothing" probes saw dead ends. Setting `var295=0` makes the hover→var68→CE245 chain work (observed).
- **End-to-end in the live map did NOT complete the rollover.** With var295=0, the hover→CE245→rest→是 sequence entered CE327 and CE328 (instances 327,328 observed) but CE328 **aborted before pc700** — var64 was never set, CE329/CE345 cleared `var69 1→0`, and the game returned to **day2 19:30 with no day change** (inst back to [13,15,25,63]). The same CE245 invoked standalone (no ev4 wrapper) did reach day3 08:30.
  → Therefore a **click-only natural route from the 19:30 bedroom all the way to next day is NOT proven**. Two concrete blockers are identified: (1) `var295` tip/overlay gate on ev4 page0; (2) CE328's pre-pc700 branch aborts in map context (missing trigger/input), plus ev4 page0's post-return `CE325` (pc340) resetting the evening.

## How the bedroom night menu is wired (SampleMapA)

- **SampleMapA ev1 page0** (name `妹クリック判定（夜`, trigger=parallel, cond var69==1 AND var25==0 AND var26==0 AND var302==0):
  - pc5 `op124 [self100, 0x4000, 120, 8]` = picture field 8 = **hover test** on **pic120** (sister sprite, `Picture/imouto02.png`, x=240 y=340, anchor=0 → hit box is the rect from (240,340) with the texture's size; transparent pixels count).
  - pc6 if hover==1 → pc8 `op123 [self101,16]` (key poll, mask 16) → pc9 if self101==10 (**CODE_OK / Decide**, see crates/wolf-game/src/key.rs:66) → **pc11 var68=1**.
  - So the "click the sister" input is **mouse hovering pic120 + pressing the Decide key**, NOT a mouse button.
- **SampleMapA ev4 page0** (name `メニュー処理`, trigger=autorun, cond **var68==1 AND var69==1 AND var62==0**, 342 cmds):
  - pc1/7/13/19/25 guard each of var295 / var167 / var26 / var439 / var464; **any ==1 → pc3 var68=0; pc4 BreakEvent**.
  - pc63 weekday branch draws HUD pics 107..120; pc70 draws pic120 + sister dialogue.
  - pc164..296: time-based branches that **auto-call CE327 (休息)** only at late hours (var4>=21/22/23 or var4<=2). At **var4==19 none fire** → continues.
  - **pc335 `op210 [500245,...]` = CE245 (夜コマンドメニュー)**; then pc338 var9=0, pc339 var68=0, **pc340 CE325 (自宅)**.

CE245 button pictures actually drawn in the map context (probe, x=512):
| pic | y | text |
|----|----|----|
| 1200/1201 | 200 | 身体接触 |
| 1202/1203 | 250 | 沟通 |
| 1204/1205 | 300 | 洗澡 |
| 1206/1207 | 350 | 泡茶 |
| 1212/1213 | 450 | 今天先休息吧 |

## var295 — the tip/overlay gate (new finding)

Writers/readers (crates/wolf-format/examples/tmp_night, mode `writes 295` / `scan '*' 2000295`):
- **set =1**: `CE326 [┗食事処理] pc1648` `op121 [2000295,1,0,0]` — inside the first-dinner skill TIPS tutorial, guarded by `self631==0` (pc1639) and `var10>=30` (pc1641); sets `self631=1` right after.
- **set =2**: `CE234 [ステータス表示] pc653` (when the tip is showing and the status screen opens).
- **set =0**: `SampleMapA ev6 p1 pc1045` (skill screen close path; guarded locally by `var295==2` at pc1033).
- **read as guard**: `SampleMapA ev1 p0 pc21/pc51` and `SampleMapA ev4 p0 pc1` (all `var295==1`).

Consequence: after the first dinner, `var295==1` disables the night menu until the player opens/close the status/skill UI (1→2→0). A debug fast-forward that skips that interaction leaves **var295 stuck at 1**, which masquerades as a "dead end".

## Sleep chain (verified standalone)

`CE327 [■休息]` pc0 `op111 [2, var69,0,op2, var69,1,op1]`:
- `var69==0` (pc1..26): 昼休憩 → sets `var4=18,var5=0,var69=1`, calls CE325/CE226/CE326 → back to 18:00 dinner loop.
- **`var69==1` (pc27 → pc29)**: comment `就寝（夜更かしモードへ）` → **CE328 (就寝前パジャマ)**.

`CE328 [┣就寝前パジャマ]` (1025 cmds): sets **var64=1** at pc700/pc727/pc825/pc964; calls **CE329** at pc990/pc993, CE332 at pc711/pc740.
`CE329 [┗就寝処理]` (444 cmds): real day rollover; **clears var69=0 at pc41** `op121 [2000069,0,0,65536,4]`.

Standalone evidence (probe `tmp_m6night_ce245`: `Game::new(work/all)`, `vars.set(4,19);set(5,30);set(160,2);set(6,1);set(69,1)`, **no var63/64**, `start_event(245)`, click (512,450), choose 是):
```
after open CE245: t=19:30 day=2 v63=0 v64=0 v65=0 v69=1 inst=[245]
click (512,450) -> choices=["是","算了"]  v69=1 inst=[236,245,327,328]
press 是 -> nested choice ["早上好。"] at f380
END: t=8:30 day=3 v63=0 v64=0 v65=0 v69=0  inst=[245,273,317,327,328,329]
```

## End-to-end in live map (the gap)

Probe `tmp_m6night_run`: reach day2 19:30 via the tmp_evening2 fast-forward, then `var63=0;var68=0;var295=0`, then hover pic120 (240,340) + Decide, then click rest (512,450)+是.
```
[A] t=19:30 day=2 v63=0 v64=0 v65=0 v68=0 v69=1 v295=0
CE245 active at frame 8 (v68=1)              <-- sister hover+Decide opens CE245
[B] menu pics: 1201 身体接触 / 1203 沟通 / 1205 洗澡 / 1207 泡茶 / 1213 今天先休息吧
[C] click (512,450) -> choices ["是","算了"] v69=1 day=2
    f0   inst includes 327,328 (CE327 + CE328 started), v64=0
    f100 inst 245,327,328, v64=0
    f200 inst=[13,15,25,63], v69=0, day=2, t=19:30 (no rollover)
[E] t=19:30 day=2 v63=0 v64=0 v65=0 v69=0
```
So in the map wrapper CE328 starts but **aborts before pc700** (var64 never becomes 1), then var69 is cleared and the bedroom is restored at the same 19:30 — the day does **not** advance. The difference from the standalone success is the ev4 page0 wrapper state (var9/var68 set, and its post-CE245 `CE325` at pc340), i.e. the sleep attempt is being unwound rather than committed.

## Key misc facts (for whoever continues)

- `var69` is only ever 0 or 1; set 1 at CE326 pc7, CE327 pc13, SampleMapA ev0 pc841; cleared 0 at CE317 pc417/pc911, CE329 pc41.
- `var63` set 1 **only** at SampleMapA ev1 **page2** pc11 `[2000063,1,0,0]` (page2 cond var64==1) — that is the "night sister click after pajamas" path; not needed for the CE245 rest route.
- `var64` set 1 **only** at CE328 pc700/pc727/pc825/pc964.
- CE245 call sites (500245): CE245 self pc461/543/554/593/626; CE246 pc2058; CE247 pc1185; CE248 pc230/534; CE290 pc113/187/197/233/265/275; **SampleMapA ev1 p0 pc64; SampleMapA ev4 p0 pc335**.
- CE327 (500327) call sites: CE245 pc820, CE290 pc439, SampleMapA ev4 p0 pc190/200/230/240/259/269/286.
- Command format used throughout: 500xxx = CallCommonEvent, 210 = call, 121 = SetVariable(dest,srcA,srcB,opflags), 124/0x4000 field8 = picture hover, 123 = key poll, 111 conditions, 401/420/499 branch cases.

## Reproduce

- `cargo run -q -p wolf-format --example tmp_night -- map SampleMapA.mps 1,4,6`
- `cargo run -q -p wolf-format --example tmp_night -- writes 295`
- `cargo run -q -p wolf-format --example tmp_night -- scan '*' 2000295`
- `cargo run -q -p wolf-format --example tmp_night -- ce 327 / ce 328 / ce 329`
- runtime probes were `crates/wolf-game/examples/tmp_m6night_run.rs` and `.../tmp_m6night_ce245.rs` (temporary, removed after use; source preserved in the probe transcripts above).

## Conclusion

- Natural **sleep mechanic** from CE245 exists and works in isolation with `var69==1` and no var63/64 injection.
- Natural **menu-open** from the bedroom exists (hover sister pic120 + Decide → var68=1 → ev4 page0 → CE245) **provided `var295==0`**; a stale `var295==1` (first-dinner tip) blocks it.
- A **complete click-only chain** 19:30 bedroom → CE245 → next day is **not demonstrated**: in the live map the rest choice enters CE327/CE328 but CE328 aborts before pc700 (`var64` never set), `var69` is cleared, and the game returns to day2 19:30. Missing trigger is inside CE328's pre-pc700 path (and/or the ev4 page0 post-CE245 CE325 reset). This is the exact place to continue.
