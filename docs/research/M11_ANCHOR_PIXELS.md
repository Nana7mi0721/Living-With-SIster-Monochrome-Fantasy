# M11 anchor pixel geometry audit (task-82)

Status: audit complete. No production source change was required. The existing
Picture::place / Picture::destination law already implements the correct geometry for
all seven editor anchors (0..6), and the negative-scale mirror added by task-60 keeps the
destination rectangle unchanged. The work here is an independent pixel-geometry test
matrix (crates/wolf-game/tests/m11_anchor_pixels.rs) and this evidence record.

## Why this audit exists

docs/UNVERIFIED.md H-ANC-1 and the M9 residual both left anchors 5 (0x5000) and 6
(0x6000) as geometry-inferred rather than retail-pixel-proven. Task-82 asked for a
minimal real fix only if the audit found an error, exact destination-rectangle tests for
codes 0..6 at several sizes plus negative scale, and an optional real capture.

## Code path audited

- crates/wolf-game/src/picture.rs:1585-1608 private fn place(anchor:u8,x:i32,y:i32,w:f32,h:f32)->[f32;4]
  applies the origin fractions in the table below.
- crates/wolf-game/src/picture.rs:1455-1466 Picture::destination takes
  self.scale.unsigned_abs()/100 for the extent, multiplies by any running breath effect,
  then calls place and transform. The sign is deliberately not part of the extent.
- crates/wolf-game/src/picture.rs:1466-1470 Picture::is_mirrored is true when scale < 0.
- crates/wolf-game/src/picture.rs:1334-1397 Screen::quads: for non-text image/panel kinds it
  resolves the texture frame and calls destination; when is_mirrored it emits one
  reversed one-pixel source column per destination column via mirrored_columns
  (picture.rs:1407-1429), otherwise one quad.
- crates/wolf-game/src/game.rs:1689 hover_picture and game.rs:1714 picture_hit_box use
  Picture::destination, so hit-testing sees the same rectangle as the draw. game.rs was not
  modified in this task.

## Anchor law and evidence

Each anchor subtracts a fraction of the SCALED extent from the x/y point:

    code  name             left subtracted   top subtracted
    0     top-left         0                 0
    1     centre           w/2               h/2
    2     bottom-left      0                 h
    3     top-right        w                 0
    4     bottom-right     w                 h
    5     top-centre       w/2               0
    6     bottom-centre    w/2               h
    7     unused fallback  0                 0

Repository evidence:
- 0 top-left: the default used by 8911 long-form draws (x=0, y=0 most often).
- 1 centre: the 4393 0x1000 draws; title Button01.png 606x35 at (512,520) renders ink
  x232..794 y504..535, i.e. both axes centred (docs/research/wolf-bytecode-crosscheck.md).
- 2 bottom-left: CE245 pc237 description text 1221 at (10,570); the bottom edge must be
  y=570, not the top.
- 3 top-right: CE217 text 6200/6211 at x=140/66 and CE216 pc190 text 6251 at x=1015
  y=7; the right edge is pinned at x.
- 4 bottom-right: docs/OPCODE_101.md:114-125 verified window command
  0x404f11 at (self42,self43)=(1024,576) with a 960x168 window gives the tagged rect
  (64,408)-(1024,576).
- 5 top-centre: CE522 pc157 draws Picture/stand_sub12.png (718x700) at (512,100), so
  left = 512-359 = 153 and top = 100. The 8 anchor-5 positive draws in the census are all
  this kind of stand art; the remaining anchor-5 occurrences are negative-number screen
  layers (CE505) where x/y and anchor are ignored.
- 6 bottom-centre: CE251 draws 482x225 icons at (800,576), so top = 576-225 = 351 and
  left = 800-241 = 559; CE245 pc236/271/... draws the 482x225 reaction icon 1220 at
  (240,576), so left = -1 and top = 351.
- 7: zero sites in this game; top-left fallback is the only safe reading.

Fresh census (work/out/anchor_probe.txt, produced for task-47/60 and retained as evidence):
anchor2=84, anchor3=115, anchor4=70, anchor5=8, anchor6=72, anchor7=0 (349 total non-corner
draws). The editor manual also lists seven base positions including top-centre and
bottom-centre (work/out/yadot_picture.html).

## Negative scale

The official WOLF help (work/out/yadot_picture.html, section 拡大率) says a negative
percentage flips the image, and its 2x2 table covers horizontal-only, vertical-only and
both-negative cases. The 150 long form carries one uniform percentage in raw slot [9], so
this engine reads a negative uniform value as a horizontal mirror at abs(scale); a
vertical-only mirror would need the separate Y percentage that the save format stores
(vars7_18[10], still an H-PIC-2 residual because Picture has no separate Y scale field).

CE245 proves the retail case: pic1220 is 482x225 at (240,576), anchor 6, scale -100. Its
destination stays [-1,351,482,225] (identical to +100), and the leftmost destination
column is clipped, so the draw list carries 481 reversed columns from source x=0 at
screen x=480 down to source x=480 at screen x=0. The source art SD_menu_013.png has the
upright girl on the right; the captured frame shows the upright girl on the left, i.e. a
horizontal mirror, not a 180-degree rotation.

## Tests added

crates/wolf-game/tests/m11_anchor_pixels.rs, 5 tests:
1. every_anchor_code_subtracts_the_right_fraction_at_every_size_and_scale: codes 0..7,
   widths {1,3,7,101,482,718}, heights {1,3,5,35,225,700}, scales {100,150,50,-100,-150},
   exact destination floats within 1e-4.
2. a_negative_scale_keeps_every_anchors_rectangle_and_only_reverses_the_columns: for
   anchors 0..6 the negative destination equals the positive one and Screen::quads emits
   four reversed columns with column 0 at the right edge and column 3 at the left.
3. real_anchor_five_stand_art_hangs_from_the_top_centre: CE522 picture/stand_sub12.png
   718x700 at (512,100) -> [153,100,718,700].
4. real_anchor_six_menu_icons_sit_on_the_bottom_edge: CE251 482x225 icon at (800,576)
   -> [559,351,482,225].
5. real_ce245_negative_scale_anchor_six_icon_is_a_mirrored_bottom_band: pic1220 scale
   -100 at (240,576) -> destination [-1,351,482,225], 481 on-screen columns, first
   src [0,0,1,225] dst [480,351,1,225], last src [480,0,1,225] dst [0,351,1,225].

## Commands and observed results

    cargo test -q -p wolf-game --test m11_anchor_pixels
    -> running 5 tests ... test result: ok. 5 passed; 0 failed

    cargo test -q -p wolf-game --lib
    -> running 231 tests ... test result: ok. 231 passed; 0 failed

    cargo test -q -p wolf-game --test m8_anchor_slots --test m8_negative_scale
    -> 4 passed and 5 passed respectively; no regression

## Capture note

There is no retail screenshot that isolates anchor 5: event 522 is the album story
display, and a smoke capture at 180 frames (work/out/m11_event522.png, 89.7% non-black,
0 warnings) shows its album list rather than the stand_sub12 art. The anchor-6 negative
case does have a real frame from task-60: work/out/m9_negative_scale_hover.png shows
pic1220 over the night menu at 18.4% non-black, 0 warnings. The exact geometry claims
above are therefore proven by the shipped command operands plus asset sizes, with the
route/model tests as the binding regression check.

## Residual

- Anchors 5 and 6 are confirmed by the real commands and asset dimensions above, but no
  retail pixel-diff frame isolates anchor 5. Their formulas are the only ones consistent
  with the editor's seven-position menu and with all 8 / 72 census sites.
- Code 7 never occurs; it stays a top-left fallback.
- Vertical-only mirror remains unrepresentable without a separate Y scale field
  (H-PIC-2 residual); this task did not add one.
- crates/wolf-game/src/textpic.rs:625-635 still documents 5/6/7 as editor-reserved and
  falls back to top-left for text pictures. No text picture in this game uses anchor 5 or
  6 (all census sites are image kind 0), so the fallback has no rendered effect; textpic.rs
  is outside this task's write scope.
- No engine change was needed and no production file was modified.
