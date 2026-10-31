# M9 W5-b - picture anchor nibbles and long-form slots [2]/[5]

Status: anchors 0..6 are implemented in crates/wolf-game/src/picture.rs (place()).
Slots [2] and [5] were already consumed there; the new test locks both.
Proof: crates/wolf-game/tests/m8_anchor_slots.rs (4 tests) and the picture unit test
place_honours_the_seven_anchor_codes.

## Problem

docs/UNVERIFIED.md H-ANC-1 said only anchor 0 (top-left) and 1 (centre) were
implemented and that codes 0x2000..0x6000 kept the corner behaviour. H-PIC-2 said
long-form slots [2] and [5] were unknown and that the renderer ignored them. The
census below shows that 349 of the game's picture sites use a non-corner code, so
leaving them as top-left misplaces every bottom description, right-aligned HUD
text and bottom menu icon.

## Anchor codes

The code is bits 12..14 of the 150 head word. PictureCmd::anchor carries it to
picture.rs. The mapping below is the final implementation:

    code 0  0x0000  top-left       x/y are the top and left edges
    code 1  0x1000  centre         x/y are both middles
    code 2  0x2000  bottom-left    x is the left edge, y is the bottom edge
    code 3  0x3000  top-right      x is the right edge, y is the top edge
    code 4  0x4000  bottom-right   x is the right edge, y is the bottom edge
    code 5  0x5000  top-centre     x is the horizontal centre, y is the top edge
    code 6  0x6000  bottom-centre  x is the horizontal centre, y is the bottom edge
    code 7          never appears; falls back to top-left

### Evidence already in the repository

- docs/OPCODE_101.md lines 100-125 tag a real 960x168 window as verified. Its
  instruction 0x404f11 (anchor 4) is placed at (self42,self43) = (1024,576) and
  the verified rectangle is (64,408)-(1024,576): both edges are subtracted.
- docs/M2_SLICE.md lines 345-349 record the verified anchor table 0 top-left,
  1 centre, 2 bottom-left, 3 top-right, 4 bottom-right, and the retail save1
  pixel match for the text path.
- crates/wolf-game/src/textpic.rs lines 625-635 already implements the same law
  for HUD text: anchor 2 = (x, y-h), 3 = (x-w, y), 4 = (x-w, y-h). Retail save1
  proves anchor 3 right-aligns on x (pic6200 x=140, ink 85..138; pic6211 x=66,
  ink 32..64).

### Fresh census of the shipped data

A temporary example (crates/wolf-format/examples/probe_anchor.rs, deleted after
use) parsed BasicData/CommonEvent.dat and every MapData/*.mps, then printed each
long-form 150 draw/move whose head nibble is 2 or more. The dump is retained as
work/out/anchor_probe.txt. Counts (common plus map):

    anchor 2: 84
    anchor 3: 115
    anchor 4: 70
    anchor 5: 8
    anchor 6: 72
    anchor 7: 0
    total non-corner: 349

Decisive samples:

- anchor 2: common event 245 pc237 draws text 1221 at (10,570), a bottom
  description. A top-left rule would put its top at 570 and clip it; a bottom
  edge at 570 leaves the 6-pixel margin the screenshots show.
- anchor 3: common event 217 draws text 6200/6211 at x=140/66 with y=7/10, and
  common event 232/234 draws HUD text at x=1000. These are right-aligned, so x
  must be the right edge.
- anchor 4: common event 251 pc1546 draws text 1211 at (1024,570), the
  bottom-right description; the verified 101 window above is the same code.
- anchor 5: common event 522 draws Picture/stand_sub12.png (718x700) at
  (512,100), stand_sub11.png (1620x668) at (512,100), stand_sub10.png
  (1024x680) at (512,100) and stand_sub05.png (324x668) at (512,100). Only a
  horizontal-centre rule lands them centred: 512-718/2 = 153 and
  512-1620/2 = -298. The negative-number samples (-5/-6/-7) are whole-screen
  layers, where position is ignored anyway.
- anchor 6: 59 draws of picture 1210 at (800,576) and 13 draws of picture 1220
  at (240,576). The files 1.4.0/SD_tetsudai2.png, SD_menu_010.png and their
  siblings are all 482x225 on disk. y=576 is exactly the logical screen bottom,
  so y is the bottom edge; x is the horizontal centre (240-482/2 = -1 fits the
  left half, 800-482/2 = 559 the right).

Anchor 5 and 6 are the only codes not independently confirmed by a retail
screenshot; they are inferred from the real draw positions and the real texture
sizes above. Anchor 7 has zero sites, so the fallback is unobservable.

## Long-form slots [2] and [5]

PictureCmd::args starts at raw instruction word 2, so raw slot [2] is args[0]
and raw slot [5] is args[3]. Both were already handled by merge_args:

- raw [2] -> args[0] -> field::FADE_IN: fade-in length in frames, 0 = instant.
  Evidence: every splash draw in the boot chain carries 20 here, and the retail
  disclaimer frame is exactly trunc(255 * age / 20) of the source art. The
  distribution (0 in 9,770 sites, 15 in 1,568, 5 in 481, 30 in 431, 100 in 242,
  10 in 210, 20 in 185, 3 in 162) is a set of plausible frame counts, not an
  opaque flag.
- raw [5] -> args[3] -> field::RESERVED: one-based sprite-sheet cell number for
  file pictures, 1 = whole image. Evidence: retail save 90 records pattern 2 for
  the 6x2 stand_imouto_paja2.png sheet and save 92 records pattern 7 for the
  7x1 eye strip; only a one-based count fits both. The distribution (1 in
  12,156, 2 in 559, 3 in 346, -1000000 in 146, 5 in 58) is the cell index, with
  -1000000 the ordinary UNCHANGED sentinel on moves.

Neither slot needs new engine behaviour; the change is that the test now pins
the meaning so a future refactor cannot silently drop them.

## Code changes (only the allowed files)

crates/wolf-game/src/picture.rs:

- Added public constants ANCHOR_BOTTOM_LEFT=2, ANCHOR_TOP_RIGHT=3,
  ANCHOR_BOTTOM_RIGHT=4, ANCHOR_TOP_CENTRE=5, ANCHOR_BOTTOM_CENTRE=6.
- place(anchor,x,y,w,h) now subtracts width and/or height per code instead of
  only for ANCHOR_CENTRE. The title path (code 1) is unchanged.
- Updated the module field table (raw slot [2] and [5] are fade and cell), the
  anchor table and the explanation, plus the Picture::anchor and field docs.
- Replaced the old unit test that asserted code 3 behaved like a corner with
  place_honours_the_seven_anchor_codes, which pins all eight cases.

crates/wolf-game/tests/m8_anchor_slots.rs (new):

1. every_anchor_code_places_the_extent_against_its_edges - all codes 0..7 and
   the real-data rectangles.
2. a_bottom_centred_icon_stays_on_screen_where_the_old_corner_rule_lost_it -
   CE251's 482x225 icon at (800,576) anchor 6 is at [559,351,482,225]; a
   top-left rule would be entirely below the screen and dropped.
3. raw_slot_two_is_the_fade_in_length - fade 20 gives alpha 0 at age 0, 0.25 at
   age 5 and 1.0 at age 20; fade 0 is instant.
4. raw_slot_five_is_the_one_based_sprite_sheet_cell - a 6x2 sheet with cell 2
   selects src [400,0,400,650], cell 7 selects [0,650,400,650], and UNCHANGED
   keeps cell 1.

## Commands and observed results

    cargo test -q -p wolf-game --lib
    231 passed; 0 failed

    cargo test -q -p wolf-game --test m8_anchor_slots
    4 passed; 0 failed

    cargo test -q -p wolf-game
    all test binaries green (exit 0)

A visual smoke capture of the affected render path:

    cargo run -q -p wolf-cli -- --root work/all capture --event 251 --frames 120 --out work/out/m9_anchor_slots.png

It renders the day command menu at 93.6 percent non-black, 0 warnings. This is
not anchor proof: event 251's own menu overlay is drawn by pictures 1200..1205,
and the anchor-6 icons 1210 are drawn later by the map event script, which this
standalone capture does not reach. The numeric assertions above are the proof.

## Residual uncertainty

- Anchor 5 and 6 have no retail screenshot; the geometry is strong but not a
  pixel diff. A future session with a retail frame showing an album stand or a
  day menu icon can close that last gap. Anchor 7 never occurs.
- crates/wolf-game/src/game.rs lines 1590-1605 duplicate the old centre-only
  placement for mouse hover hit-testing. That file is outside this task's write
  scope, so non-centre-anchored pictures still hit-test as if top-left. This is
  a real residual click-region bug for scripts that hover those pictures.
- CE245 draws its anchor-6 night-menu icons (picture 1220) with scale = -100.
  Picture::destination currently clamps scale at 0, so those icons have zero
  extent and are not drawn, anchor or not. Whether retail reads a negative
  scale as a mirror is unresolved and separate from this anchor fix.
