# M9 W5-b window skin completeness and nine-slice regression

Task: enumerate the real type-3/type-4 window pictures in the shipped data, prove the
nine-slice geometry and the 0x0 fallback, confirm the actual SystemFile/WindowBase.png
pixels, and cover more than one simultaneous window. Result: no production bug was
found in crates/wolf-game/src/window.rs or the atlas insertion path, so no production
file was changed. The new regression test is crates/wolf-game/tests/m8_window_skin.rs.

## 1. Window picture model

picture.rs decodes opcode 150 into two window kinds:

    code 3 WindowFile    literal file name
    code 4 WindowString  file name taken from a string reference

Only these two kinds return true from PictureKind::is_window. The box lives in
Picture.window as Option<(i32, i32)>. picture.rs merge_args writes it only for
non-file kinds, using args[1] as width and args[2] as height (PictureCmd.args starts
at raw argument 2):

    Some((w, h)) when w > 0 and (h > 0 or (h == 0 and the file is a SystemGraphic))
    None otherwise

So a type-4 window with no box stays None (the 0x0 fallback), while a type-3
SystemGraphic can carry a self-sizing box of (w, 0).

The skin name is picture.window_skin(): the picture file when it is non-empty, does
not start with < and is not a SystemGraphic; otherwise the default
SystemFile/WindowBase.png. Built-in SystemGraphic drawings such as <TRI-RIGHT> are
explicitly drawn as sprites by Screen::quads and are never used as panels.

## 2. Data census

Every opcode 150 command whose kind is 3 or 4 was extracted from
BasicData/CommonEvent.dat and all nine MapData/*.mps. The working tree roots
work/all and work/full produced identical counts:

    source                         sites
    CommonEvent.dat                 349
    MapData/SampleMapA.mps          185
    MapData/SampleMapA_1.mps        133
    total                           667

    kind 3 WindowFile               576
    kind 4 WindowString              91
    action 0 Draw                   455
    action 1                         3
    action 2                        72
    action 3 Set                    137
    file is a SystemGraphic <...>   402
    file is empty                   265
    distinct literal picture numbers 62

Raw dump: work/out/window_census.tsv (667 command rows). The temporary extraction
example used to produce the census was deleted before completion.

Most window commands do not carry a literal picture number or box at all: their
number and box are self variables such as 1600020/1600021/1600040, so the runtime
number differs from the raw word. This is why a static scan finds 62 literal
numbers but only one fixed literal title window.

### Fixed literal windows

    #12500  CE94 pc251, kind 4, action 0, number 12500, width 300, height 200,
            string ref 1600008, empty file
This is the only fixed literal window picture in the shipped data. It is the title
save/load backdrop at destination (855, 200), size 300x200.

    #12005  runtime type 3 WindowFile, file <TRI-RIGHT>, box (1600011, 1600012)
The real trace shows Set with no args followed by Draw with args
[0,1600011,1600012,1,255,1600057,1600058,100,0,...]. The box is self sizing; at
the observed point both values are zero, so no panel is painted and the triangle is
drawn as a normal SystemGraphic.

    #12505  absent
No literal #12505 exists in CommonEvent.dat or any MapData .mps, in either data
root. The task example was stale on this point. #12502 does exist but is kind 1
(FileString), not a window, so it is out of scope for the skin path.

## 3. Nine-slice algorithm

crates/wolf-game/src/window.rs defines:

    SKIN = SystemFile/WindowBase.png
    CORNER = 3
    MIN_BOX = 2
    PANEL_PICTURE = -100

nine_slice(dst, skin_w, skin_h) returns an empty vector for a zero-size skin or a
non-positive destination width/height. Otherwise it cuts cw = min(3, skin_w / 2)
and ch = min(3, skin_h / 2), splits each axis into [0, corner, len - corner, len]
and emits the nine pieces, skipping any piece whose source or destination is empty.
For the default 18x18 skin and a 300x200 destination at (100, 50) the measured
pieces are:

    top-left      rect [100, 50, 3, 3]       src [0, 0, 3, 3]
    centre        rect [103, 53, 294, 194]   src [3, 3, 12, 12]
    bottom-right  rect [397, 247, 3, 3]      src [15, 15, 3, 3]

The nine destination areas sum to exactly 300x200 and the nine source areas cover
18x18 exactly once. The four 3x3 corners are not stretched; only the middle is.

atlas.rs builds one panel per visible window picture: it skips non-window kinds,
SystemGraphic files, boxes at or below MIN_BOX and off-screen destinations, then
adds the skin asset and pushes one SpriteQuad per nine_slice piece. Quads are sorted
by picture number, so several windows are rendered in a stable order.

## 4. Actual skin pixels

SystemFile/WindowBase.png is exactly 18x18 in both data roots. The measured grid:

    row 0     corners transparent; the rest 0x424242
    row 1     0x424242 edge, 0x888888 corner, 0xf2f3f3 top bevel
    row 2     0x424242 edge, 0xf2f3f3 bevel, 0xd9dcdd corner, 0x576062 transition
    rows 3-14 col0 0x424242, col1 0xf2f3f3, col2 0x576062,
              body cols 3-14 0x1f2425 with alpha 0xdf,
              col15 0x576062, col16 0xf2f3f3, col17 0x424242
    row 15    mirror of row 2
    row 16    mirror of row 1
    row 17    mirror of row 0

Thus the task-cited colours are real and measurable: 0x424242 outer border,
0xf2f3f3 highlight bevel, 0x1f2425 translucent body. The corner texels also carry
0x888888 and 0xd9dcdd, and the inner transition line is 0x576062.

## 5. Runtime confirmation

A temporary probe started event 42 from work/full and ran 300 frames. From frame
210 the screen contained real windows:

    #999   WindowFile <SQUARE>, window (100, 50)
    #6231  WindowFile <SQUARE><FRAME>, window (150, 15)
    #6232  WindowFile <SQUARE>, window (150, 14)
    #6233  WindowFile <SQUARE>, window (150, 14)
    #19500 WindowString SystemFile/WindowBase.png, window (960, 168), at (512, 480)

The probe observed warnings = 0. The temporary probe test was deleted before
completion.

## 6. Regression test

crates/wolf-game/tests/m8_window_skin.rs contains five tests:

    the_real_skin_grid_has_the_measured_three_pixel_frame            line 80
    a_normal_300x200_window_uses_3px_corners_and_a_stretched_middle line 117
    zero_box_windows_and_the_type3_12005_triangle_do_not_paint_a_panel line 133
    several_windows_at_once_each_get_their_own_nine_slice_in_number_order line 164
    the_shipped_data_census_has_667_window_commands_and_no_literal_12505 line 197

The tests cover: the exact 18x18 pixel grid; the 300x200 geometry and area
conservation; type-3 #12005 with a zero box and type-4 zero box producing no panel;
the 1x1 sentinel being rejected by MIN_BOX; three simultaneous windows producing 27
quads sorted by number; and the 667-site census with 576/91 kind counts, one fixed
#12500 in CE94 and zero literal #12505.

## 7. Commands and results

    cargo run -q -p wolf-format --example _tmp_window_census -- work/all
        -> 667 rows: 349 CE, 185 SampleMapA.mps, 133 SampleMapA_1.mps;
           576 type 3, 91 type 4 (worked identically on work/full)

    cargo test -q -p wolf-game --test m8_window_skin -- --nocapture
        -> ok. 5 passed; 0 failed; 0 ignored

    cargo check -q --workspace
        -> exit 0 (only the pre-existing dead_code warning on
           crates/wolf-game/src/battle_wiring.rs:185 set_command_schedule)

## 8. Residual risk

H-WIN-1 remains a visual-only risk, not a geometry bug. The CN patch text picture
#12501 is placed at (715, 115) with adaptive size about 309x461, while the shipped
#12500 window is 300x200 at (855, 200); the text is not nested in the window and its
right edge exceeds the 1024-wide screen. Rendering keeps the shipped coordinates
faithfully. If that patch is meant to run on this 1024-wide data, the data placement
is the thing to fix, not window.rs.

No literal #12505 is present in the current data; if a future data revision adds
one, it will use the same type-3/type-4 and MIN_BOX rules. Self-sized and
self-numbered windows still need runtime values, so a static census cannot replace
a live trace for those rows.
