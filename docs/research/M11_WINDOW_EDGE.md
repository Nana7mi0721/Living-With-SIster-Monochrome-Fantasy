# M11 W5 window-edge: the CN title save/load patch

Status: investigated at HEAD 22de104. The CN patch is a data placement that already
fits the 1024x576 screen. No production change was needed in window.rs or atlas.rs;
the suspected right-edge overflow came from reading the window origin as a top-left
corner and ignoring the command's centre anchor.

## What was claimed

docs/research/M9_WINDOW_SKIN.md section 8 recorded:

- #12500 is a 300x200 window at (855, 200), so its right edge is 855 + 300 = 1155 > 1024.
- #12501 is text at (715, 115) with adaptive size about 309x461, not nested in the window.

That arithmetic treats (855, 200) as a top-left destination. It is not: picture 12500
carries anchor code 1 (centre), and Picture::destination subtracts half the box before
placing it. The actual destination is [705, 100, 300, 200], whose right edge is 1005.

## Runtime evidence

Real capture command:

    ./target/debug/wolf-cli.exe --root work/all capture --event 94 --frames 120 --out work/out/task87_event94.png

CE94 is the shipped save/load screen draw event ('X[移]セーブ·ロード画面描画'). At frame
120 the capture reports the patch live:

    pic 12500  WindowString at (855, 200) alpha 1.00 scale 100 window Some((300, 200)) file "SystemFile/WindowBase_amania.png"
    pic 12501  Text at (715, 115) alpha 1.00 scale 100 window Some((1, 1)) text "欢迎来到Ver2.0DLC！..."

The screenshot work/out/task87_event94.png is 1024x576. The dark panel's right edge is
at x=1005, with a 19 px black margin before x=1024, and every Chinese text line is well
inside the panel.

## Exact geometry

| item | value |
| --- | --- |
| #12500 raw origin | (855, 200) |
| #12500 anchor | 1 (centre) |
| #12500 box | 300x200 |
| #12500 destination | [705, 100, 300, 200] |
| #12500 right / bottom | 1005 / 300 |
| #12500 skin | SystemFile/WindowBase_amania.png, 18x18 |
| #12501 raw origin | (715, 115) |
| #12501 anchor | 0 (top-left) |
| #12501 window | Some((1, 1)) self-sizing sentinel |
| text runs | 7 lines |
| text extents | x=715..961 (capture), y=115..297 |

Nine-slice pieces for the panel destination with the real 18x18 skin:

    top-left      [705, 100,   3,   3]
    centre        [708, 103, 294, 194]
    bottom-right  [1002, 297,  3,   3]

All nine pieces satisfy rect inside [705, 100, 300, 200], and therefore inside
1024x576. The actual atlas path produces exactly those nine quads; the first is
[705, 100, 3, 3] and the last [1002, 297, 3, 3].

The text picture is not nested inside the panel, but its measured extents fit:

    [715, 115, 210, 26] 欢迎来到Ver2.0DLC！
    [715, 141, 100, 26] 烦请注意
    [715, 167, 140, 26] 存档的第一栏
    [715, 193, 180, 26] 为自动存档所使用
    [715, 219, 240, 26] 第一栏已有存档数据时，
    [715, 245, 180, 26] 请先读取该存档，
    [715, 271, 240, 26] 重新保存至合适的位置。

Max right is 955 in Game::text_quads (the capture's glyph boxes add one font-px of
slack, max right 961); both are below the panel right 1005 and the screen right 1024.
Bottom is 297, below the panel bottom 300 and the screen bottom 576.

## Why M9 was wrong

M9 used a static census of the raw 150 command, which records number 12500, box
300x200 and raw x=855, y=200, but not the runtime anchor nibble. The census cannot
turn an origin into a destination without the anchor. The runtime Picture for #12500
reports anchor=1, so 855 is the horizontal centre and 200 the vertical centre:
left = 855 - 300/2 = 705 and top = 200 - 200/2 = 100. There is no off-screen panel.

## Code path (unchanged)

1. CE94 draws picture 12500 with a 300x200 window box and picture 12501 as text.
2. crates/wolf-game/src/picture.rs:1455 Picture::destination applies place(self.anchor, ...);
   anchor 1 subtracts half width and half height.
3. crates/wolf-game/src/atlas.rs:531 uses the same destination and splits it with
   crates/wolf-game/src/window.rs:62 window::nine_slice.
4. No path clips the panel to a smaller box, but here no clipping is required because
   the destination is wholly on screen. The renderer's normal screen bounds handle
   any future off-screen panel.

## Regression test

New file crates/wolf-game/tests/m11_window_edge.rs, three tests:

- the_cn_patch_panel_is_centred_so_its_right_edge_is_1005_not_1155
  asserts kind WindowString, raw (855,200), anchor 1, box (300,200), destination
  [705,100,300,200], right 1005, and every nine-slice piece inside panel and screen.
- the_cn_patch_text_runs_stay_inside_the_panel
  asserts #12501 is top-left text at (715,115), 7 runs, each run inside the panel
  rectangle and the screen, with max right 955 and bottom 297.
- the_real_atlas_draws_the_cn_patch_panel_with_quads_inside_the_screen
  runs Atlas::sprites on the live screen and asserts exactly nine #12500 quads, each
  inside the panel rectangle and the screen, with right edge <= 1024.

Commands and results:

    cargo test -q -p wolf-game --test m11_window_edge
        -> ok. 3 passed; 0 failed

    cargo test -q -p wolf-game --lib
        -> ok. 234 passed; 0 failed

## Residual

- No production edit means there is no before/after diff to show; both sides are the
  same HEAD 22de104 build. The capture and tests record the actual current output.
- #12501 depends on the loaded font metrics. The test runs with the shipped font and
  checks the real seven runs; a system without the retail font may lay the text out
  differently, but the string is still short enough to fit the 300x200 panel.
- This proves the CN patch itself. Other CE94 menu elements are self-variable sized and
  are out of this window-edge question; the capture starts CE94 alone, so only the
  fixed #12500/#12501 patch is asserted here.
