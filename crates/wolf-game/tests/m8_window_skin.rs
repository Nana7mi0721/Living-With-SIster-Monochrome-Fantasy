//! M9 W5-b window skin completeness and nine-slice regression.
//!
//! The shipped data has two window picture kinds:
//!
//!   type 3 WindowFile   - a window whose file is named literally
//!   type 4 WindowString - a window whose file comes from a string variable
//!
//! A real panel is painted for a window picture only when the picture has a usable
//! box (both sides greater than window::MIN_BOX) and its file is not a built-in
//! SystemGraphic drawing. The panel is the 18x18 SystemFile/WindowBase.png split
//! with the measured 3 px frame and a stretched middle.
//!
//! The census in docs/research/M9_WINDOW_SKIN.md enumerates every opcode 150
//! kind-3/kind-4 command in BasicData/CommonEvent.dat and every MapData .mps:
//! 667 command sites, 576 type 3 and 91 type 4. Most carry a self-variable picture
//! number or box (1600020/1600021/1600040), so the runtime number is not the raw
//! literal. The only fixed literal title window is #12500 (type 4, 300x200 at
//! 855,200); #12005 is a type-3 SystemGraphic triangle with a zero box and must
//! be drawn as a drawing, never as a panel. No literal #12505 exists anywhere in
//! either data root; #12502 is a type-1 file picture, not a window.
//!
//! This file pins the geometry, the 0x0 fallback, the actual skin pixels and the
//! multiple-window path. window.rs and the atlas insertion path already implement
//! these rules; no production change was needed.

use std::path::PathBuf;

use wolf_game::assets::{Assets, ImageData};
use wolf_game::atlas::Atlas;
use wolf_game::picture::{PictureKind, Screen, ANCHOR_TOP_LEFT};
use wolf_game::window;
use wolf_vm::exec::PictureAction;
use wolf_vm::PictureCmd;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn pixel(image: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}

fn picture(number: i32, kind: u8, file: &str, args: &[i32]) -> PictureCmd {
    PictureCmd {
        number,
        kind,
        action: PictureAction::Draw,
        file: file.to_owned(),
        text: String::new(),
        args: args.to_vec(),
        animate: Vec::new(),
        anchor: ANCHOR_TOP_LEFT,
    }
}

fn skin_image(fill: u8) -> ImageData {
    ImageData::new(18, 18, vec![fill; 18 * 18 * 4]).expect("18x18 skin image")
}

fn pack_skin(atlas: &mut Atlas) {
    atlas
        .add(window::SKIN, &skin_image(0x7f))
        .expect("pre-pack the 18x18 skin");
}

#[test]
fn the_real_skin_grid_has_the_measured_three_pixel_frame() {
    let path = data_root().join("SystemFile").join("WindowBase.png");
    if !path.is_file() {
        eprintln!("skipping: {} is missing", path.display());
        return;
    }
    let image = image::open(&path)
        .expect("decode SystemFile/WindowBase.png")
        .to_rgba8();
    assert_eq!(image.dimensions(), (18, 18), "the skin is an 18x18 nine-slice asset");

    // The four extreme corners are fully transparent: the frame only starts one
    // texel in, which is why the 3 px cut includes the empty corner.
    for (x, y) in [(0, 0), (17, 0), (0, 17), (17, 17)] {
        assert_eq!(pixel(&image, x, y), [255, 255, 255, 0], "corner {x},{y} is transparent");
    }
    // The task-cited colours are all present, at the measured positions:
    // 0x424242 is the outermost border run, 0xf2f3f3 the highlight bevel and
    // 0x1f2425/0xdf the translucent body.
    assert_eq!(pixel(&image, 5, 0), [0x42, 0x42, 0x42, 0xff], "top outer border");
    assert_eq!(pixel(&image, 0, 5), [0x42, 0x42, 0x42, 0xff], "left outer border");
    assert_eq!(pixel(&image, 5, 1), [0xf2, 0xf3, 0xf3, 0xff], "top highlight bevel");
    assert_eq!(pixel(&image, 1, 5), [0xf2, 0xf3, 0xf3, 0xff], "left highlight bevel");
    assert_eq!(pixel(&image, 5, 2), [0x57, 0x60, 0x62, 0xff], "top transition line");
    assert_eq!(pixel(&image, 2, 5), [0x57, 0x60, 0x62, 0xff], "left transition line");
    assert_eq!(pixel(&image, 5, 5), [0x1f, 0x24, 0x25, 0xdf], "body fill");
    assert_eq!(pixel(&image, 14, 14), [0x1f, 0x24, 0x25, 0xdf], "body centre");
    assert_eq!(pixel(&image, 5, 15), [0x57, 0x60, 0x62, 0xff], "bottom transition line");
    assert_eq!(pixel(&image, 5, 16), [0xf2, 0xf3, 0xf3, 0xff], "bottom highlight bevel");
    assert_eq!(pixel(&image, 5, 17), [0x42, 0x42, 0x42, 0xff], "bottom outer border");
    // The corner 3x3 differs from the edge 3x3 only at its first texel; that is
    // why the 3 px frame cannot be reproduced by stretching a single row.
    assert_eq!(pixel(&image, 1, 1), [0x88, 0x88, 0x88, 0xff], "corner bevel");
    assert_eq!(pixel(&image, 2, 2), [0xd9, 0xdc, 0xdd, 0xff], "corner transition");
}

#[test]
fn a_normal_300x200_window_uses_3px_corners_and_a_stretched_middle() {
    let pieces = window::nine_slice([100.0, 50.0, 300.0, 200.0], 18, 18);
    assert_eq!(pieces.len(), 9, "18x18 skin yields nine pieces");
    assert_eq!(pieces[0].rect, [100.0, 50.0, 3.0, 3.0], "top-left");
    assert_eq!(pieces[0].src, [0, 0, 3, 3]);
    assert_eq!(pieces[4].rect, [103.0, 53.0, 294.0, 194.0], "stretched centre");
    assert_eq!(pieces[4].src, [3, 3, 12, 12]);
    assert_eq!(pieces[8].rect, [397.0, 247.0, 3.0, 3.0], "bottom-right");
    assert_eq!(pieces[8].src, [15, 15, 3, 3]);
    let dst_area: f32 = pieces.iter().map(|p| p.rect[2] * p.rect[3]).sum();
    assert!((dst_area - 300.0 * 200.0).abs() < 1e-3, "area {dst_area}");
    let src_area: u32 = pieces.iter().map(|p| p.src[2] * p.src[3]).sum();
    assert_eq!(src_area, 18 * 18, "every skin texel is used once");
}

#[test]
fn zero_box_windows_and_the_type3_12005_triangle_do_not_paint_a_panel() {
    // The real #12005 is a built-in triangle: a type-3 command whose picture box is
    // self-variable sized (and zero at this point). It must become a drawing, never
    // nine stretched skin quads.
    let mut screen = Screen::new();
    screen.apply(&picture(12005, 3, "<TRI-RIGHT>", &[0, 0, 0, 1, 255, 100, 100, 100, 0]));
    let p = screen.get(12005).expect("12005 exists");
    assert_eq!(p.kind, PictureKind::WindowFile);
    assert_eq!(p.window, None, "zero box stays None; MIN_BOX also rejects 1x1");
    let mut atlas = Atlas::with_size(128, 128);
    let skin = atlas
        .add(window::SKIN, &skin_image(1))
        .expect("skin");
    let mut assets = Assets::new("no-such-root");
    let quads = atlas.sprites(&screen, &mut assets);
    assert!(quads.iter().all(|q| q.number == 12005));
    assert_eq!(atlas.entry(window::SKIN), Some(skin), "no new skin pack was requested");

    // A type-4 window with an explicit 0x0 box is the other fallback spelling.
    screen.apply(&picture(12500, 4, "", &[0, 0, 0, 1, 255, 100, 100, 100, 0]));
    assert_eq!(screen.get(12500).unwrap().window, None, "0x0 type-4 box is not a panel");
    assert!(atlas.sprites(&screen, &mut assets).is_empty(), "no panel and no content");
    assert_eq!(atlas.entry(window::SKIN), Some(skin), "skin was not packed again");

    // The 1x1 sentinel WOLF writes for self-sizing text is rejected by MIN_BOX.
    screen.apply(&picture(12501, 4, "", &[0, 1, 1, 1, 255, 100, 100, 100, 0]));
    assert_eq!(screen.get(12501).unwrap().window, Some((1, 1)));
    assert!(atlas.sprites(&screen, &mut assets).is_empty(), "1x1 is below MIN_BOX");
}

#[test]
fn several_windows_at_once_each_get_their_own_nine_slice_in_number_order() {
    let mut screen = Screen::new();
    // #10: 300x200 at (100,50); #20: 200x100 at (500,300); #5: 100x80 at (700,100).
    screen.apply(&picture(10, 4, "", &[0, 300, 200, 1, 255, 100, 50, 100, 0]));
    screen.apply(&picture(20, 4, "", &[0, 200, 100, 1, 255, 500, 300, 100, 0]));
    screen.apply(&picture(5, 4, "", &[0, 100, 80, 1, 255, 700, 100, 100, 0]));
    let mut atlas = Atlas::with_size(64, 64);
    pack_skin(&mut atlas);
    let mut assets = Assets::new("no-such-root");
    let quads = atlas.sprites(&screen, &mut assets);
    assert_eq!(quads.len(), 27, "three windows x nine pieces");
    assert!(quads.iter().all(|q| (q.alpha - 1.0).abs() < 1e-6));
    assert!(quads.iter().all(|q| q.tint == [1.0, 1.0, 1.0]));
    assert_eq!(
        quads.iter().map(|q| q.number).collect::<Vec<_>>(),
        std::iter::repeat(5).take(9)
            .chain(std::iter::repeat(10).take(9))
            .chain(std::iter::repeat(20).take(9))
            .collect::<Vec<_>>(),
        "panels and their content are sorted by picture number"
    );
    let centres: Vec<(i32, f32, f32, f32)> = quads
        .iter()
        .filter(|q| q.rect[2] > 3.0 && q.rect[3] > 3.0)
        .map(|q| (q.number, q.rect[0], q.rect[1], q.rect[2]))
        .collect();
    assert_eq!(centres.len(), 3, "one stretched middle per window");
    assert_eq!(centres[0], (5, 703.0, 103.0, 94.0), "#5 middle");
    assert_eq!(centres[1], (10, 103.0, 53.0, 294.0), "#10 middle");
    assert_eq!(centres[2], (20, 503.0, 303.0, 194.0), "#20 middle");
}

#[test]
fn the_shipped_data_census_has_667_window_commands_and_no_literal_12505() {
    use wolf_format::command::Slot;
    use wolf_format::commonevent::CommonEventData;
    use wolf_format::mps::Map;

    let root = data_root();
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.clone()
    };
    let ce_path = basic.join("CommonEvent.dat");
    if !ce_path.is_file() {
        eprintln!("skipping: {} is missing", ce_path.display());
        return;
    }
    let bytes = std::fs::read(&ce_path).expect("read CommonEvent.dat");
    let events = CommonEventData::parse(&bytes).expect("parse CommonEvent.dat");
    let mut total = 0usize;
    let mut type3 = 0usize;
    let mut type4 = 0usize;
    let mut literal_12500 = Vec::new();
    let mut literal_12505 = 0usize;
    for event in &events.events {
        for slot in &event.commands {
            let Slot::Command(cmd) = slot else { continue };
            if cmd.opcode != 150 {
                continue;
            }
            let words: Vec<i32> = cmd
                .params
                .chunks_exact(4)
                .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
                .collect();
            if words.len() < 2 {
                continue;
            }
            let kind = (words[0] >> 4) & 7;
            if kind != 3 && kind != 4 {
                continue;
            }
            total += 1;
            if kind == 3 {
                type3 += 1;
            } else {
                type4 += 1;
            }
            let number = words[1];
            let width = words.get(3).copied().unwrap_or(0);
            let height = words.get(4).copied().unwrap_or(0);
            if kind == 4 && number == 12500 && width == 300 && height == 200 {
                literal_12500.push(event.id);
            }
            if number == 12505 {
                literal_12505 += 1;
            }
        }
    }
    let dir = root.join("MapData");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().ends_with(".mps") {
                continue;
            }
            let Ok(bytes) = std::fs::read(entry.path()) else { continue };
            let Ok(map) = Map::parse(&bytes) else { continue };
            for event in &map.events {
                for page in &event.pages {
                    for slot in &page.commands {
                        let Slot::Command(cmd) = slot else { continue };
                        if cmd.opcode != 150 {
                            continue;
                        }
                        let words: Vec<i32> = cmd
                            .params
                            .chunks_exact(4)
                            .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
                            .collect();
                        if words.len() < 2 {
                            continue;
                        }
                        let kind = (words[0] >> 4) & 7;
                        if kind != 3 && kind != 4 {
                            continue;
                        }
                        total += 1;
                        if kind == 3 {
                            type3 += 1;
                        } else {
                            type4 += 1;
                        }
                        let w = words.get(3).copied().unwrap_or(0);
                        let h = words.get(4).copied().unwrap_or(0);
                        if words[1] == 12500 && kind == 4 && w == 300 && h == 200 {
                            literal_12500.push(event.id);
                        }
                        if words[1] == 12505 {
                            literal_12505 += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(total, 667, "shipped opcode-150 window command sites");
    assert_eq!(type3, 576, "type-3 WindowFile sites");
    assert_eq!(type4, 91, "type-4 WindowString sites");
    assert_eq!(literal_12500, vec![94], "only CE94 draws the fixed 300x200 #12500");
    assert_eq!(literal_12505, 0, "no literal #12505 exists in the shipped data");
}
