# M9 hover hit-testing: anchor-aware opcode 124 field 8

Status: implemented, tested, no commit (run at HEAD bcbcfba on top of the M9 anchor work).

## Problem

Opcode 124 field 8 (docs/OPCODE_124.md) asks whether the mouse is over a picture. The
headless implementation lived in `GameHost::hover` in `crates/wolf-game/src/game.rs` and
duplicated the renderer's placement maths by hand. It subtracted half width/height only
when `picture.anchor == ANCHOR_CENTRE`; every other anchor kept the top-left corner. It also
sized a picture from the whole texture, ignoring a sprite-sheet split.

After M9 gave `picture::place` the full seven-code anchor map, the two paths disagreed. The
common night-menu icon is 482x225 drawn at (800, 576) with the bottom-centre anchor: the
renderer draws it in y 351..576, but the old hover put its top edge at y=576, so the whole
visible icon was unclickable while a row of empty screen below it was. Top-right HUD text
and bottom-left descriptions had the mirror-image error.

## Fix

`crates/wolf-game/src/game.rs`:

- `GameHost::hover` (game.rs:1614) now delegates to `hover_picture(self.screen,
  self.assets, self.mouse, number)`.
- `fn hover_picture` (game.rs:1626) returns -1 when the slot is absent or has no hit box,
  otherwise 1 when the mouse is inside the destination rectangle and also inside the
  1024x576 screen, 0 otherwise. Transparent pixels still count: the hit area is the
  rectangle, as the engine documents.
- `fn picture_hit_box` (game.rs:1651) reproduces the renderer's extent: a screen layer
  covers [0,0,1024,576]; text uses its window box; a window kind uses its window when both
  sides exceed `window::MIN_BOX`; everything else uses `picture_texture_extent`. The final
  rectangle is `Picture::destination(width, height)`, so anchor, percent scale,
  positional effects and the screen anchor all match what `Screen::quads` draws.
- `fn picture_texture_extent` (game.rs:1671) reads `assets.texture_size` and divides by the
  `split` columns/rows to get one cell, so a sprite sheet is hit-tested at cell size; a
  built-in system graphic falls back to `sysgraphic::parse` + `box_size`.
- `pub fn Game::picture_hover(&mut self, number) -> i32` (game.rs:774) exposes the same
  routine to headless tests without duplicating the maths.

`Picture::destination` (crates/wolf-game/src/picture.rs:1396) already applied
`place(anchor, x, y, w, h)` (picture.rs:1516) with this map:

| code | anchor | origin |
| --- | --- | --- |
| 0 | top-left | (x, y) |
| 1 | centre | (x-w/2, y-h/2) |
| 2 | bottom-left | (x, y-h) |
| 3 | top-right | (x-w, y) |
| 4 | bottom-right | (x-w, y-h) |
| 5 | top-centre | (x-w/2, y) |
| 6 | bottom-centre | (x-w/2, y-h) |
| 7 | fallback top-left | (x, y) |

No renderer or picture.rs change was needed for this task.

## Tests

New file `crates/wolf-game/tests/m8_hover_anchor.rs`, five tests:

1. `every_anchor_code_hit_tests_the_drawn_rectangle` builds a window-kind picture per
   anchor, asserts `destination` equals the expected rectangle, then one point inside hits
   and one point just outside misses. Covers codes 0..6.
2. `a_bottom_centred_icon_at_the_screen_bottom_is_clickable_in_its_visible_band` uses the
   real CE245 geometry 482x225 at (800, 576): y=566 and y=361 hit, y=350 misses, and the
   drawn top (351) is proven above the screen bottom (576).
3. `a_top_right_hud_picture_hit_tests_against_the_right_edge` uses 200x40 at x=1000:
   x=800 hits, x=999 hits, x=1000 and x=799 miss.
4. `a_sprite_sheet_cell_is_hit_tested_at_cell_size_not_sheet_size` builds a 2x1 split on a
   real skin texture, asserts the drawn width is half the sheet, hits inside the first cell
   and misses past the cell edge.
5. `a_real_day_command_menu_icon_is_still_clickable` starts common event 251, runs 120
   frames, finds icons 1200/1202/1204 centre-anchored at (512, 230/280/330), and asserts
   (512,230) hits 1200, (512,280) hits 1202, but icon 1200 does not cover the next row;
   warnings are 0.

Commands and results:

```
cargo test -q -p wolf-game --test m8_hover_anchor
running 5 tests ... 5 passed; 0 failed

cargo test -q -p wolf-game --test m6_night_menu --test m6_training_menu \
  --test m6_mainline --test m7_hscene_entry
7 + 2 + 2 + 3 passed; 0 failed

cargo test -q -p wolf-game --lib
running 231 tests ... 231 passed; 0 failed
```

The m7_hscene_entry suite is the real CE245 end-to-end click: it drives the canonical
day-2 evening loop, opens the night command menu and clicks a night-menu row, so the
anchor-aware hit test is exercised on the real CE245 icons as well as the CE251 icons in
test 5.

Probe output from event 251 (icon rows): `[(-10, File, 0, 0, 0), (1200, File, 1, 512, 230),
(1201, Text, 1, 512, 230), (1202, File, 1, 512, 280), (1203, Text, 1, 512, 280), (1204,
File, 1, 512, 330), (1205, Text, 1, 512, 330), ...]` (number, kind, anchor, x, y).

## Residuals

- Anchor codes 5 and 6 are evidenced by data geometry and the shared text anchor law, not by
  a retail pixel screenshot; see docs/research/M9_ANCHOR_SLOTS.md. The hit test uses the same
  map as the renderer, so a future anchor correction only needs to land in
  `picture::place` and this code follows through `Picture::destination`.
- `game.rs` hover still counts transparent pixels (documented engine behaviour), and
  negative-scale mirroring remains unresolved in the renderer, so those pictures are
  hit-tested at their clamped destination.
- No commit or push; the Lead integrates.
