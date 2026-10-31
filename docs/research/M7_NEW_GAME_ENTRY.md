# M7 W4 — Real new-game entry: title → CE42 → Map003 → SampleMapA

Status: measured (HEAD `1d36ebe`, data root `work/all` / `work/full`).
Owner: m6-night. Scope: read-only except `docs/research/M7_NEW_GAME_ENTRY.md` and
`crates/wolf-game/tests/m7_new_game_entry.rs`. No production edits, no commit.

Question: when a retail player starts a new game, what is the actual map chain, and
where do the start coordinates come from?

Short answer: the chain is **TitleMap event 2 → CE42 menu option 1 → Map003 event 1 →
SampleMapA at (62, 38)**. Both coordinates are authored as literal operands of opcode
`130` in the two scripts; `MapTree.dat` is **not** the source of the position (it stores
only `parent`/`map_id`, no x/y) and no engine code reads it.

## Reproduction

```
cd 'E:/Study/github program/imoto life'
source tools/env.sh
# the tests for this investigation:
cargo test -q -p wolf-game --test m7_new_game_entry -- --nocapture
# the full retail boot -> new game route, rendered:
cargo run -q -p wolf-cli -- --root work/all capture --boot \
  --frames 3200 --key-frame 800:10 --answer 0 --answer-delay 2 \
  --out work/out/m7_new_game_entry.png
```

Observed test output:

```
running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s
```

The temporary `--example tmp_newgame` probe that first dumped CE42/CE549 was removed
after writing this report; its byte-level findings live in the static test below.

## 1. The route, hop by hop

Opcode `130` operands are `[target, x, y, map, options]`
(`crates/wolf-vm/src/exec.rs:1792-1800` calls `host.teleport(arg0, arg3, arg1, arg2, arg4)`;
`crates/wolf-game/src/game.rs:1728-1736` pushes `MapRequest{target, map, x, y, options}`).

| # | where | script slot | `130` operands | meaning |
|---|-------|-------------|----------------|---------|
| 1 | boot | `TitleMap.mps` event 2 | (splash/title art via CE549 `forSteam[caution]`, 65 cmds, 3 splash calls) | draws title |
| 2 | `CommonEvent` 42 `オリジナルタイトル画面Ａ` | pc 118 | `[-1, 60, 34, 3, 1]` | target hero, x=60, y=34, map id **3** (`Map003`), options 1 |
| 3 | `Map003.mps` event 1 `ニューゲーム処理` | pc 28 | `[-1, 62, 38, 1, 1]` | target hero, x=62, y=38, map id **1** (`SampleMapA`), options 1 |

CE42 is CommonEvent id 42, title `オリジナルタイトル画面Ａ`, 135 commands,
`raw_condition = 32`. It draws the menu at pc 113 (`op102` with strings
`["1", "2", "3"]`), branches on `op401[2]` at pc 114, and on the new-game answer
issues the teleport at **pc 118**, then `op401[3]` at pc 120 and `op210 [500014, 1, -1]`
at pc 123. `BASELINE.json:1071` records the same `pc118 130 [-1, 60, 34, 3, 1]`.

`Map003.mps` event 1 (`id == 1`, name `ニューゲーム処理`) issues its single teleport at
**pc 28** with `[-1, 62, 38, 1, 1]`. `Map003` event 0 (`画像消去`) only clears pictures.

At runtime the engine logs each switch with the exact strings (from `game.rs:1082-1108`):

```
130 Teleport: MapData/Map003.mps at (60, 34) target -1 options 0x1
130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1
```

Note the coordinates are logged as the literal operands, because `switch_map` takes no
position (`crates/wolf-game/src/game.rs:663-673`): this engine switches the map and lets
the destination map's own events place/dress the scene. That is sufficient here because
`SampleMapA` event 0 runs its new-game opening unconditionally on entry.
## 2. What the test proves

`crates/wolf-game/tests/m7_new_game_entry.rs` has two tests:

1. `the_new_game_hops_have_exact_static_operands` (deterministic, parses the shipped
   data): CE42 title contains `タイトル`, its `op102` menu is exactly `["1", "2", "3"]`,
   `teleports(ce42.commands) == [(118, [-1, 60, 34, 3, 1])]`; `Map003` event 1 name
   contains `ニューゲーム` and `teleports(ev1.pages[0].commands) == [(28, [-1, 62, 38, 1, 1])]`;
   `MapTree.dat` parses to 50 entries and `children_of(0)` contains 3.
2. `the_retail_title_menu_boots_a_new_game_through_map003_into_sample_map_a` (runtime,
   the player path): boots the title, confirms with `key::OK`, reads the CE42 menu
   `["1", "2", "3"]`, answers option 1, and then asserts both exact `130` diagnostics
   above, the `SampleMapA` opening prompt
   `["广阔的战场", "广袤的森林", "微风吹拂而过的麦田", "有人在等我的家"]`,
   zero warnings, `instructions > 10_000`, and that no negative `TitleGraphic` picture
   survives the switch. Crucially it never calls `start_map_event_by_name`, so a pass
   proves the retail chain, not a synthetic start at `Map003` event 1.

End-to-end rendered capture (`work/out/m7_new_game_entry.png`, 1024x576):

```
rendered  : 1024x576, 542737/589824 non-black pixel(s) = 92.0%
stats     : 192993 instruction(s), 9443 picture op(s), 10 sound(s),
            979 db access(es) (295 write, 330 label), 39 message(s), 6 choice(s),
            0 warning(s)
background: 108 registered (10 always + 1 auto_start + 97 conditional);
            17 distinct common event(s) started, 17 start(s) total
```

The 9 distinct diagnostics are benign: the two `130` hops above plus a `106` debug line,
`160/162` hard-cut transition, `2x 201 SetMoveRoute: ... not applied (no map layer yet)`,
`0x 220 SaveLoad: no template save`, `250 DatabaseAccess: CData (5, 16, 0) ... unanswered`,
`2x 270 Party: 1 argument(s) recorded only`, and `290 Effect: kind 10 ... not drawn yet`.
None indicate a broken new-game chain.

## 3. `MapTree.dat` is an ordering tree, not a start position

`MapTree.dat` (416 bytes, 50 entries) stores only `parent: i32` and `map_id: u32` per
entry (`crates/wolf-format/src/maptree.rs`). Root is `parent == -1`; the three children
of root 0 are map ids `3`, `31`, `24`. **There is no x/y field anywhere in the file**, and
`grep` finds zero references to `MapTree`/`maptree` in `crates/wolf-game` — the engine
never reads it. Therefore any claim that the new-game start coordinate comes from
`MapTree.dat` is unsupported; `(62, 38)` is hard-coded in `Map003` event 1 pc 28, and
`(60, 34)` in CE42 pc 118.

## 4. Boundary / dead ends

* Do not prove this route with `start_map_event_by_name("Map003", 1)`: that skips CE42
  and the title entirely (the existing `opening.rs` test does exactly this and is a
  synthetic unit check). The runtime test above is the one that exercises CE42.
* `130`'s logged coordinates are magic operands, not decoded positions; `switch_map`
  ignores position. This engine relies on destination-map events to place the player,
  which works for `SampleMapA` event 0 but would not by itself move a sprite.
* `MapTree`'s root children include `31`/`24` (the tile-less maps) as well as `3`; the
  tree does not encode which child is the new-game start — only `Map003` event 1 does.

## 5. Deliverables

* `crates/wolf-game/tests/m7_new_game_entry.rs` — the two tests above (223 lines).
* `docs/research/M7_NEW_GAME_ENTRY.md` — this report.
* `work/out/m7_new_game_entry.png` — 92.0% non-black rendered capture of the route.

Re-run: see `## Reproduction`. No production source was modified; nothing was committed.
