//! Optional text overlay: replace the shipped 101 Message strings at load time.
//!
//! The game data stays byte-for-byte untouched.  A caller that sets
//! WOLF_TEXT_OVERLAY to a table produced by work/translation/gen_override.py
//! gets the fan-translation Chinese drawn instead, while every other command,
//! variable reference and escape code remains the original.  Rows are keyed by
//! (common event id, slot index), the same pc the interpreter executes, so the
//! replacement cannot drift if the table is regenerated.
//!
//! File format: one record per line, event<TAB>pc<TAB>text, # comments and blank
//! lines ignored.  text is single-line with backslash, \n and \t escaped, so a
//! message that itself contains a newline survives the round trip.

use std::collections::BTreeMap;
use std::path::Path;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;

/// Environment variable naming the override table to apply on load.
pub const ENV: &str = "WOLF_TEXT_OVERLAY";

/// A parsed (event, pc) -> text table.
#[derive(Debug, Clone, Default)]
pub struct TextOverlay {
    entries: BTreeMap<(u32, usize), String>,
}

impl TextOverlay {
    /// Parse a table from its text form.
    #[must_use]
    pub fn parse(src: &str) -> Self {
        let mut entries = BTreeMap::new();
        for line in src.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(3, '\t');
            let (Some(event), Some(pc), Some(text)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let (Ok(event), Ok(pc)) = (event.trim().parse::<u32>(), pc.trim().parse::<usize>())
            else {
                continue;
            };
            let text = unescape(text);
            if !text.is_empty() {
                entries.insert((event, pc), text);
            }
        }
        Self { entries }
    }

    /// Read a table from disk.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing was parsed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Replace the matching 101 message strings in place, returning how many slots
    /// were rewritten.  A (event, pc) row whose slot is not a non-empty 101 command
    /// is skipped, never panics: the table and the data are separate files and can
    /// drift.
    pub fn apply(&self, events: &mut CommonEventData) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        let mut applied = 0usize;
        for event in &mut events.events {
            for (pc, slot) in event.commands.iter_mut().enumerate() {
                let Some(replacement) = self.entries.get(&(event.id, pc)) else {
                    continue;
                };
                if let Slot::Command(command) = slot {
                    if command.opcode == 101 && !command.strings.is_empty() {
                        command.strings[0] = replacement.clone();
                        applied += 1;
                    }
                }
            }
        }
        applied
    }
}

/// Load the table named by ENV, if the variable is set.
pub fn load_from_env() -> std::io::Result<Option<TextOverlay>> {
    match std::env::var_os(ENV) {
        Some(path) if !path.is_empty() => Ok(Some(TextOverlay::load(Path::new(&path))?)),
        _ => Ok(None),
    }
}

/// Undo the generator's single-line escaping.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::TextOverlay;

    #[test]
    fn parses_rows_and_ignores_comments_and_garbage() {
        let table = TextOverlay::parse(
            "# comment\n\n280\t8\t\\\\Ehello\nbad line\n281\tx\tnope\n282\t3\t\n",
        );
        assert_eq!(table.len(), 1);
        assert_eq!(table.entries.get(&(280, 8)).map(String::as_str), Some("\\Ehello"));
    }

    #[test]
    fn unescape_restores_newlines_tabs_and_backslashes() {
        let table = TextOverlay::parse("1\t2\tline1\\nline2\\ttab\\\\Eend\n");
        assert_eq!(
            table.entries.get(&(1, 2)).map(String::as_str),
            Some("line1\nline2\ttab\\Eend")
        );
    }
}
