# M9 W1 · Production multi-round battle driver on the BOSS path

Task: shared task-57 (owner m6-night2). Repo HEAD `bcbcfba` (all work uncommitted, Lead integrates).
Write scope: `crates/wolf-game/src/battle_wiring.rs`, `crates/wolf-game/src/game.rs`,
`crates/wolf-game/tests/m8_battle_boss_rounds.rs`, this document. No commit/push.

## 1. Problem statement

In every *shipped* battle the enemy gauge `2000342` is seeded to `party_power / 8` (CE239
`pc195`: `121 [2000342, 2000341, 1600011, 0]`, with `2000341 = 2000028 / 4` at `pc194` and
`1600011 = 2000341 / 2` at `pc193`). A single player command deals far more damage than
`party_power/8`, so round 1 zeroes the gauge and CE244 `┗終了判定` always settles there.
There is simply nothing for a multi-round driver to chain on the shipped path — the
round-loop from task-49 was provably inert.

The other half of the shipped blocker: `2000350` (command phase) reaches `2` at the end of a
CE242 slot body, and the only normal-path writer that resets it `2 -> 0` is CE241
`┣ターン開始処理` `pc628`. But CE241 is a boot-once `ENGINE_AUTO_START = &[241]` event
(`crates/wolf-game/src/game.rs:98`): it runs once at game start, clears its own condition and
exits. Nothing re-arms it, so after round 1 the flow sits at phase `2` forever.

## 2. The real BOSS branch of CE239 (`■戦闘呼び出し`, 1108 cmds)

CE239 case 1 seeds the enemy gauge, then overrides/boosts it for a **boss** battle. Exact
commands (event index = pc):

```
pc180  121 [2000341, 1600070, 1600071, 0]      ; 2000341 = party_power - enemy_atk
pc193  121 [1600011, 2000341, 2, 12288]        ; 1600011 = 2000341 / 2
pc194  121 [2000341, 2000028, 4, 12544]        ; 2000341 = party_power / 4
pc195  121 [2000342, 2000341, 1600011, 0]      ; 2000342 = party_power/8  (normal gauge)
pc199  121 [2000342, 2000341, 0, 0]            ; else-branch gauge
pc202  111 [1, 2000445, 1, 2]                  ; IF 2000445 == 1  -> BOSS case
pc204  121 [2000342, 2000446, 0, 256]          ; BOSS gauge uses 2000446
pc205  111 [1, 2000342, 100, 3]                ; IF 2000342 <= 100
pc207  121 [2000342, 100, 0, 0]                ;   clamp gauge to 100 (floor)
```

So `2000445 == 1` selects a BOSS battle whose enemy gauge is derived from `2000446`
(empirically the initial gauge is `2000446` plus the small `2000341` base — e.g. seeding
`2000446 = 4000` gives an observed initial gauge of `4529`). A large enough `2000446` survives
the first command round, which is exactly what makes a real multi-round fight reachable.

## 3. CE244 `┗終了判定` settlement gate (what must open / stay shut)

Decision head (event index = pc):

```
pc0   121 [1600000, 0]                 ; dispatch scratch
pc3   121 [1600010, 11]                ; default turn threshold = 11
pc36  111 [1, 2000349, 1600010, 2]     ; IF 2000349 == turn_threshold -> 「战斗结束」
  pc44 111 [17, 2000192, 2000193, 0]   ;   IF party_gauge > enemy_gauge -> 勝利 else 敗北
pc54  111 [1, 2000215, 1, 2]           ; special branch
pc77  111 [18, 2000342, 0, 3, 2000029, 0, 3]  ; case1 enemy gauge<=0 / case2 party<=0
pc85  111 [17, 2000029, 0, 3]          ; IF party<=0 -> 献身スキル block
```

Victory block `pc312-317`: `2000350=0`, `2000342=0`, `1600000=1`, `2000352=1`, `2000343=1`.
Defeat block `pc370-375`: `1600000=-1`, `2000352=-1`, `2000343=1`, `2000342=0`, `2000350=0`
(alt `-2` at `pc415-420`). `2000349` is only ever written to `1` (CE239 `pc564`, CE241 `pc78`),
so the `pc36` turn-threshold arm normally stays shut — a boss fight ends by draining the enemy
gauge (`pc77` case 1 → victory) or the party pools (`2000345..348` → defeat), both real.

## 4. The fix — engine driver using only real events

`battle_wiring.rs` already carries the task-49 round loop. task-57 exposes it to a running
game and proves it on the BOSS path:

* `crates/wolf-game/src/game.rs` — new `pub fn set_battle_round_loop(&mut self, enabled: bool)`
  (default OFF; `self.battle_wiring.set_round_loop(enabled)`) and a read-only
  `pub fn battle_wiring_turn_starts(&self) -> u64` for tests. Off by default ⇒ a game that
  never calls it behaves byte-for-byte like the shipped single-round battle.
* `BattleWiring::tick` round-loop (opt-in via `set_round_loop(true)`):
  1. on the `2000340` rising edge (`350==0`, no 241/242/243 alive) start the next CE242 command
     body (`start_command`, rotating `command_schedule`),
  2. when `350==2` and no 241/242/243 instance is alive, start the **real** CE241 turn-start
     (its `pc628` resets `350 2 -> 0`), incrementing `turn_starts`,
  3. when `350==0` again with no round event alive and `started_rounds > 0`, start the next
     CE242 command body.

No state is faked: the driver never writes `2000350 = 1`, never sets `2000352`/`2000343`, and
never injects the settlement result. It only *starts real events* (CE242 / CE241) at the
moments the shipped 3-state machine needs them, exactly as the retail script intends.

## 5. Test evidence — `crates/wolf-game/tests/m8_battle_boss_rounds.rs`

Opens the real SaveData01 battle chain (CE253 → CE239), seeds BOSS context **before** the chain
runs (`2000445 = 1`, `2000446 = 4000`), enables wiring + round loop, and traces the phase
sequence, rounds, turn-starts and party pools. Assertions: `rounds >= 2`, `turn_starts >= 1`,
a `2 -> 0` transition in the phase sequence (proof CE241 fired), at least two distinct member
pools drained, `2000343 == 1`, `2000352 in {1,-1}`, `warnings == 0`.

Observed (current tree, opcode-111 patch **not** applied — see §6):

```
BOSS v350_seq=[0, 1, 2, 0, 1, 2, 0, 1, 0]
BOSS pre=[5, 4, 3, 2] post=[3, 3, 3, 2] rounds=3 turn_starts=2
     max242_pc=967 v352=1 v343=1 settled=true warnings=0
```

The phase sequence `... 2, 0, 1, 2, 0 ...` is the real round cycle: CE242 reaches `2`, CE241
resets to `0`, the next CE242 runs `1 -> 2`. Command schedule `[1, 3]` drains two different
members — pool `2000345` (cmd 1) and `2000346` (cmd 3) both drop (`pre [5,4,3,2]` ->
`post [3,3,3,2]`). The fight ends in a real victory (`2000352 = 1`, `2000343 = 1`).

With the **parked** opcode-111 value-operand patch applied (`exec.rs` comparing `eval_num`), the
same test also passes, just with a different but still-real round count (`rounds=5`,
`v352=1`); the driver is robust across both interpreter semantics.

Run:

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_boss_rounds -- --nocapture
```

## 6. Regression status and the shared-worktree caveat

At the time of writing, individually:

| suite | result |
|-------|--------|
| `m8_battle_boss_rounds` (new) | 1 passed |
| `m8_battle_party` | 2 passed |
| `m7_battle_playable` | 1 passed |
| `m7_day4_playable` | 1 passed |
| `m7_battle_target_scheduler` | 6 passed |

**Caveat (important for integration):** during one combined run `m7_battle_target_scheduler`
`::battle_wiring_advances_to_phase_two_but_ce244_gate_blocks_settlement` failed with
`left: 0, right: 2`. This was *not* caused by the round-loop (which is OFF unless
`set_battle_round_loop(true)` is called). It is the documented, sole consequence of the
**parked opcode-111 value-operand patch** (`docs/research/M9_OPCODE_111_REGRESSION.md`,
failure row #2): `crates/wolf-vm/src/exec.rs` was momentarily carrying the `eval_num(...)`
patch (a concurrent teammate's revert↔restore experiment). Once `exec.rs` returned to HEAD
(no `eval_num` at the opcode-111 value operand), all four suites above pass green. This file
and the new test do not depend on that patch either way.

## 7. Minimal-fix summary (what the Lead should land)

1. `crates/wolf-game/src/battle_wiring.rs`: keep the task-49 round-loop driver (already present
   in the working tree; round_loop defaults OFF).
2. `crates/wolf-game/src/game.rs`: keep `Game::set_battle_round_loop` + `battle_wiring_turn_starts`.
3. `crates/wolf-game/tests/m8_battle_boss_rounds.rs`: new regression proving ≥2 real rounds on
   the BOSS path.

Production boot only needs `Game::set_battle_wiring(true)` **and** (if a genuine multi-round
boss fight should chain) `Game::set_battle_round_loop(true)`. Because both default OFF, no
existing behaviour changes until a caller opts in.

## 8. Boundaries / risk

* The driver only chains rounds for a battle whose enemy gauge outlives round 1 — in shipped
  data that is the BOSS branch (`2000445 == 1`). Ordinary battles still settle round 1.
* The `command_schedule` defaults to `[1, 3]`; it is not yet driven by the player's live
  command-menu selection (that is a separate input-wiring task). The schedule is a *test
  driver* for the party loop, not the production player choice path.
* `set_command_schedule` is currently unused by production code (`#[warn(dead_code)]`); future
  command-input wiring should thread the real selection through it.
* No `docs/BASELINE.json`, `docs/HANDOFF_M4.md`, source beyond the three declared files, or the
  3 permanent scratch files were touched. No commit/push.