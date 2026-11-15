//! UI-08: option and message text must resolve database variables before measuring.
//!
//! Opcode 102 stores option strings with their escapes intact. The choice panel used to
//! measure and paint the raw source, so a label like
//! `你是……\cdb[0:12:0]\cdb[0:12:1]的哥哥。` showed the control codes and could be
//! truncated because the panel was sized around invisible characters.

use wolf_game::choice;
use wolf_game::message;
use wolf_game::textpic;

struct FakeVars;

impl textpic::Vars for FakeVars {
    fn num(&self, _index: usize) -> i32 {
        0
    }

    fn text(&self, _index: usize) -> String {
        String::new()
    }
}

struct FakeCells;

impl textpic::Cells for FakeCells {
    fn cdb(&self, ty: i32, row: i32, column: i32) -> Option<String> {
        match (ty, row, column) {
            (0, 12, 0) => Some("妹妹".to_owned()),
            (0, 12, 1) => Some(String::new()),
            _ => None,
        }
    }

    fn udb(&self, _ty: i32, _row: i32, _column: i32) -> Option<String> {
        None
    }
}

struct FakeColours;

impl textpic::Colours for FakeColours {
    fn colour(&self, _index: i32) -> Option<[u8; 3]> {
        Some([255, 255, 255])
    }
}

impl message::Colours for FakeColours {
    fn colour(&self, _index: u32) -> Option<[u8; 3]> {
        Some([255, 255, 255])
    }
}

#[test]
fn an_option_resolves_database_variables_and_is_not_truncated() {
    let option = "你是……\\cdb[0:12:0]\\cdb[0:12:1]的哥哥。";
    let visible = "你是……妹妹的哥哥。";
    let vars = message::Vars {
        x: 64.0,
        y: 410.0,
        font_px: 20.0,
        chars_per_second: 0,
        preroll: 0,
        width: None,
        cursor_offset: None,
        screen_w: 1024.0,
    };
    let text_vars = FakeVars;
    let cells = FakeCells;
    let colours = FakeColours;
    let ctx = textpic::Ctx {
        vars: &text_vars,
        cells: &cells,
        colours: &colours,
        default_font_px: 20.0,
    };

    // The message styled path: exactly the visible text, no control codes.
    let layout = message::layout_styled_with(option, &vars, 0, &colours, Some(&ctx));
    assert_eq!(layout.rows.len(), 1, "the label fits on one row");
    assert_eq!(layout.rows[0].text, visible);
    assert!(!layout.rows[0].text.contains("cdb"));

    // The choice panel sizes itself from the same resolved text. Its text column must
    // be at least as wide as the painted label, otherwise the tail is clipped.
    let panel = choice::layout_at_sized_with(&[option.to_owned()], 0, 100, 200, 20.0, Some(&ctx))
        .expect("panel is on screen");
    let painted_width = choice::text_width_at(visible, 20.0);
    assert!(
        panel.lines[0][2] >= painted_width,
        "choice row is {} px but the label paints at {painted_width} px",
        panel.lines[0][2]
    );
}
