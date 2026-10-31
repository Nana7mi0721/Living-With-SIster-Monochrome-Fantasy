# M9 W5-b - negative picture scale is a horizontal mirror

Status: implemented in `crates/wolf-game/src/picture.rs`; regression matrix in
`crates/wolf-game/tests/m8_negative_scale.rs` (5 tests, green). No commit was made.

## The bug

`Picture::destination` used to compute the extent with `self.scale.max(0) / 100`.
A command that asked for `-100` therefore became width 0 and height 0, and
`on_screen` dropped the quad. Common event 245 draws its reaction illustrations
that way, so the night command menu silently lost one full picture whenever the
player hovered a menu row.

## What the format and data prove

1. The official WOLF editor help says a negative zoom flips the image:
   `http://yado.tk/wolf/02_ibekoma/1000_picture/`, saved decoded as
   `work/out/yadot_picture.html`. Line 1543 documents the separate horizontal/
   vertical zoom option, line 1547 states
   「拡大率をマイナス値にすると、画像が反転します。」 (a negative zoom flips the
   image), and the table at lines 1552-1559 lists 横-100/縦100 and 横100/縦-100 as
   the two mirror cases. It is not a zero-size or not-drawn sentinel.
2. The long picture command carries ONE uniform scale, not two. In this game's
   150 commands that is raw slot [9] = `SCALE` (percent). `docs/OPCODE_SPEC.md:832-849`
   lists values 100 (x12308), -100 (x474), 150 and 250 for that slot; -1000000 is
   only the UNCHANGED sentinel in slots [5]/[6]/[9]/[10].
3. Common event 245 (title 「■夜コマンドメニュー」) is the decisive live case. Its
   op-150 draws at pc236/271/275/310/342/374/405 are identical except for the file:
   `args = [3170304, 1220, 0, 1, 1, 1, 255, 240, 576, -100, 0, 0, ...]`.
   `3170304 = 0x306000` (type 0, command 0, anchor nibble 6 = bottom-centre), so the
   picture is `number 1220, x=240, y=576, scale=-100, anchor=6`. The seven files are
   `1.4.0/SD_menu_010.png`, `013`, `002`, `003`, `007`, `004`, `006`, and every one is
   482x225 on disk. The menu chooses one of them according to the hovered row.
4. A rectangle `x=240, y=576`, width 482, height 225 with anchor 6 (bottom-centre)
   lands at `[-1, 351, 482, 225]`: it hangs exactly on the bottom edge and is centred
   on x=240. Turning the sign into width 0 erases a picture retail shows; turning it
   into a horizontal mirror at 100% keeps that rectangle and only reverses the pixels.

Because the uniform command value cannot express a vertical-only flip, and because
the CE245 reaction art is upright (not upside down), the defensible reading is: a
negative uniform scale mirrors the HORIZONTAL axis at `abs(scale)`. A vertical mirror
requires the separate vertical percentage that a save record keeps (save record
vars7_18[10]); that remains H-PIC-2, not this command path.

## Implementation

`crates/wolf-game/src/picture.rs`:

- Module docs (line 94) record the evidence and the chosen semantics.
- `Picture::destination` (line 1455) now uses
  `let scale = self.scale.unsigned_abs() as f32 / SCALE_ONE as f32;` so the sign
  never changes the rectangle. Hit-testing uses the same rectangle.
- `Picture::is_mirrored` (line 1466) is the single test for `scale < 0`.
- `Screen::quads` (line 1381) branches on `is_mirrored()` and calls
  `mirrored_columns` (line 1407). For each source column it emits one 1-pixel-wide
  `ScreenQuad` at the mirrored destination column: source column 0 lands at the
  right edge, the last column at the left edge. The row and full height are kept.

### Why column decomposition instead of a UV flip

This task's write scope excludes `crates/wolf-game/src/atlas.rs`, and the existing
renderer makes a real UV flip impossible there anyway:
`ScreenQuad.src` is `[x,y,w,h]` with positive w/h, `AtlasRect::uv`/`sub_uv` always
return `u0 < u1`, and `atlas.rs` `snap()` clamps every destination width to
`max(1.0)`. A negative destination width or a reversed UV would be rounded away.
The column path is therefore the smallest correct mechanism that stays inside
`picture.rs`: it samples the same texture, produces the exact horizontal mirror,
and leaves the destination rectangle identical to the hit-test rectangle. It is a
workaround, not a native atlas mirror.

## Regression proof

New file `crates/wolf-game/tests/m8_negative_scale.rs`, five tests:

1. `a_negative_scale_keeps_the_bottom_centre_rectangle_and_mirrors_the_columns`
   - positive and negative scale both return destination `[-1.0, 351.0, 482.0, 225.0]`;
   - 482 source columns produce 481 visible quads (the leftmost source column lands
     fully off-screen at x=-1); first quad `src [0,0,1,225]` / `dst [480,351,1,225]`,
     last `src [480,0,1,225]` / `dst [0,351,1,225]`; for every quad
     `src_x + dst_x == 480.0`, so the column map is exactly `i -> width-1-i`.
2. `the_ce245_reaction_command_draws_the_mirrored_icon_on_screen` applies the exact
   `PictureCmd { number:1220, file:"1.4.0/SD_menu_010.png", args:[0,0,0,1,255,240,576,-100,0],
   anchor:ANCHOR_BOTTOM_CENTRE }` and asserts the stored fields, the destination and
   the first/last mirrored column.
3. `a_mirrored_sprite_sheet_flips_the_selected_cell_only` uses a 6x2 sheet at cell 2:
   400 quads, first source `[400,0,1,650]` at `dst [399,0,1,650]`, last source
   `[799,0,1,650]` at `dst [0,0,1,650]`, so only the selected cell is reversed and
   the sheet UV origin is respected.
4. `the_mirrored_rectangle_is_what_the_hover_test_uses` loads the real game with
   `Game::new(work/all)`, inserts picture 1220 (-100, anchor 6, x240 y576), and
   checks `Game::picture_hover`: mouse `[400,400]` is a hit (1), mouse `[600,400]`
   is outside the mirrored band (0). This is the renderer/hit-test agreement.
5. `a_zero_scale_remains_invisible` proves zero is still invisible; only the sign
   is treated as a mirror.

Commands and observed results:

- `cargo test -q -p wolf-game --lib` -> 231 passed, 0 failed.
- `cargo test -q -p wolf-game --test m8_negative_scale` -> 5 passed, 0 failed.
- `cargo run -q -p wolf-cli -- --root work/all capture --event 245 --frames 180
  --pointer 512,250 --out work/out/m9_negative_scale_hover.png` -> real CE245
  capture with a hovered row. It reports `pic 1220 File at (240, 576) alpha 1.00
  scale -100 ... file "1.4.0/SD_menu_013.png"` and its quads begin
  `quad pic 1220 at [480,351,1,225]` and descend one pixel at a time to
  `[0,351,1,225]`; `rendered 1024x576, 108499/589824 non-black = 18.4%`, `0 warning(s)`.
  The screenshot is `work/out/m9_negative_scale_hover.png`.
- Without `--pointer` the same event renders 10.7% non-black and no picture 1220;
  the reaction icon appears only when a menu row is hovered, which is why the
  pointer is part of the capture command. `work/out/m9_negative_scale.png` is the
  no-hover smoke capture.

## Residuals

- This is a horizontal mirror only. A uniform command scale cannot ask for a
  vertical-only mirror; the separate save-record vertical percentage remains H-PIC-2.
- The implementation is a per-column decomposition (up to one quad per source pixel
  wide; 481 for CE245, 400 for a 400-pixel sprite cell). It is exact for integer
  destination widths and nearest sampling, but it is not a native atlas UV flip and
  will be slightly more expensive than one quad. A future atlas flip flag would let
  `picture.rs` emit one mirrored quad instead.
- The mirror case is proven by the official manual plus shipped command geometry and
  file sizes; this repository has no retail pixel diff of the same icon in both
  orientations. The orientation choice (horizontal rather than vertical) is the
  only reading consistent with an upright reaction illustration.
- Anchor 5/6 and the `game.rs` hover path were resolved in earlier tasks; this change
  reuses `Picture::destination`, so a mirrored picture hit-tests the same rectangle
  the renderer draws (test 4 pins that).
- `docs/UNVERIFIED.md` H-PIC-2 was not in this task's write scope, so it still
  describes the old slot doubt; this document is the current evidence for the
  horizontal-mirror semantics.
