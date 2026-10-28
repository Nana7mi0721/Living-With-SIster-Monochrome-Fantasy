//! `.mps` map loader.
//!
//! Layout is documented in `docs/FORMAT_SPEC.md` §5: a fixed header (magic, version,
//! title, tileset index, size, event count), an optional tile block of three
//! `width * height` layers, then the event stream and a one-byte footer.
//!
//! Two properties of the real data are preserved rather than "repaired":
//!
//! * Two of the 134 shipped map pages end with a `0x00` terminator instead of the
//!   documented `0x7A` (`Dungeon.mps` at offset 17716, `Map024.mps` at offset 234).
//!   The loader accepts those and keeps the raw byte in [`MapEventPage::footer`]
//!   so callers can still tell them apart from healthy pages.
//! * Event names are editor-truncated (e.g. `"妹クリック判定（夜"`); they are decoded
//!   lossily and never normalised.

use crate::bytes::Reader;
use crate::command::{RouteData, Slot, SlotStats};
use crate::error::{Error, Result};

/// Number of tile layers in a `0x66` map file. The spec allows a `layer_count`
/// field from v3.5 on, but every map in this game uses the fixed three-layer format.
pub const LAYER_COUNT: usize = 3;

/// A parsed `.mps` map.
#[derive(Debug, Clone)]
pub struct Map {
    /// Header byte at offset 16, `0x55` in every shipped map.
    pub version_header: u8,
    /// Map format version at offset 24, `0x66` in every shipped map.
    pub version: u8,
    /// Map name from the editor; only ever the literal `"なし"` ("none") in this game.
    pub name: String,
    /// Index into `TileSetData.dat`, see [`crate::tileset::TileSetData::get`].
    pub tileset_index: u32,
    /// Map width in tiles.
    pub width: u32,
    /// Map height in tiles.
    pub height: u32,
    /// `false` when the tile block is just the `0xFFFFFFFF` "no layer" marker.
    /// `Map024` and `Map031` are the two such maps.
    pub has_tile_data: bool,
    /// Three layers of `width * height` raw tile words, layer 0 (surface) first,
    /// then layer 1 (decoration), then layer 2 (top), row-major within each layer.
    /// Empty when [`Self::has_tile_data`] is `false`.
    pub layers: Vec<Vec<u32>>,
    /// Map events, in file order.
    pub events: Vec<MapEvent>,
    /// Footer byte, `0x66`.
    pub footer: u8,
}

impl Map {
    /// Ten zero bytes followed by `WOLFM\0`.
    pub const MAGIC: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x57, 0x4F, 0x4C, 0x46, 0x4D,
        0x00,
    ];
    /// Three zero bytes between the version header and `unknown1`.
    pub const MAGIC2: [u8; 3] = [0x00, 0x00, 0x00];
    /// `unknown1` at offset 20; always 100 in this game's maps.
    pub const UNKNOWN1: u32 = 0x64;
    /// Map version bytes this loader understands. Both share the same layout.
    pub const VERSIONS: [u8; 2] = [0x65, 0x66];
    /// Tile word that marks "no tile" and, as the first word of the tile block,
    /// "this map has no tile layers at all".
    pub const EMPTY_TILE: u32 = 0xFFFF_FFFF;
    /// First byte of an event record.
    pub const EVENT_MAGIC: u8 = 0x6F;
    /// Second word of an event record; always 12345.
    pub const EVENT_MAGIC2: u32 = 12_345;
    /// Word between the page count and the first page; always 0.
    pub const EVENT_SEPARATOR: u32 = 0;
    /// Event footer byte.
    pub const EVENT_FOOTER: u8 = 0x70;
    /// First byte of a page record.
    pub const PAGE_MAGIC: u8 = 0x79;
    /// Documented page footer byte.
    pub const PAGE_FOOTER: u8 = 0x7A;
    /// Footer byte that two shipped pages carry instead of [`Self::PAGE_FOOTER`].
    pub const PAGE_FOOTER_TOLERATED: u8 = 0x00;
    /// Map footer byte.
    pub const FOOTER: u8 = 0x66;

    /// Parse a whole `.mps` file.
    ///
    /// Consumes the complete buffer: the event stream must be followed by the
    /// footer and then nothing but zero padding.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        r.expect(&Self::MAGIC, "map magic")?;
        let version_header = r.u8()?;
        r.expect(&Self::MAGIC2, "map magic2")?;
        let unknown1 = r.u32()?;
        if unknown1 != Self::UNKNOWN1 {
            return Err(Error::field(
                "map unknown1",
                r.pos() - 4,
                format!("expected {}, got {unknown1}", Self::UNKNOWN1),
            ));
        }
        let version = r.u8()?;
        if !Self::VERSIONS.contains(&version) {
            return Err(Error::field(
                "map version",
                r.pos() - 1,
                format!("unsupported map version 0x{version:02X}"),
            ));
        }
        let name = r.t_str()?;
        let tileset_index = r.u32()?;
        let width = r.u32()?;
        let height = r.u32()?;
        let event_count = r.u32()? as usize;
        // An event record is at least 29 bytes, so a wildly larger count cannot be
        // real. The bound below is deliberately looser than that: it only has to
        // stop a corrupt header from driving the loop, never reject real data.
        if event_count > r.remaining() / 8 {
            return Err(Error::field(
                "map event count",
                r.pos() - 4,
                format!("{event_count} events do not fit in {} bytes", r.remaining()),
            ));
        }

        let first_pixel = r.u32()?;
        let (has_tile_data, layers) = if first_pixel == Self::EMPTY_TILE {
            (false, Vec::new())
        } else {
            let pixels = (width as u64) * (height as u64) * (LAYER_COUNT as u64) * 4;
            // The first word has already been consumed above.
            let needed = pixels.saturating_sub(4);
            if needed > r.remaining() as u64 {
                return Err(Error::eof(r.pos(), needed as usize, r.remaining()));
            }
            let per_layer = (width as usize) * (height as usize);
            let mut layers = Vec::with_capacity(LAYER_COUNT);
            for layer in 0..LAYER_COUNT {
                let mut values = Vec::with_capacity(per_layer);
                if layer == 0 {
                    values.push(first_pixel);
                }
                while values.len() < per_layer {
                    values.push(r.u32()?);
                }
                layers.push(values);
            }
            (true, layers)
        };

        let mut events = Vec::new();
        while !r.at_end() && r.peek_u8()? == Self::EVENT_MAGIC {
            events.push(MapEvent::read(&mut r)?);
        }
        if events.len() != event_count {
            return Err(Error::field(
                "map event count",
                r.pos(),
                format!(
                    "header declares {event_count} events, found {}",
                    events.len()
                ),
            ));
        }

        let footer = r.u8()?;
        if footer != Self::FOOTER {
            return Err(Error::magic(
                "map footer",
                r.pos() - 1,
                &[footer],
                &[Self::FOOTER],
            ));
        }
        while !r.at_end() {
            if r.u8()? != 0 {
                return Err(Error::trailing(r.pos() - 1, r.remaining() + 1));
            }
        }

        Ok(Self {
            version_header,
            version,
            name,
            tileset_index,
            width,
            height,
            has_tile_data,
            layers,
            events,
            footer,
        })
    }

    /// Command statistics over every page of every event.
    pub fn stats(&self) -> SlotStats {
        let mut stats = SlotStats::default();
        for event in &self.events {
            for page in &event.pages {
                stats.extend(page.commands.iter());
            }
        }
        stats
    }

    /// Raw tile word of `layer` at `(x, y)`, if both are inside the map.
    pub fn tile(&self, layer: usize, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = y as usize * self.width as usize + x as usize;
        self.layers.get(layer)?.get(index).copied()
    }
}

/// A map event: the position on the map plus one page per configured state.
#[derive(Debug, Clone)]
pub struct MapEvent {
    /// Event id, unique within the map.
    pub id: u32,
    /// Event name as typed in the editor (may be truncated, see the module docs).
    pub name: String,
    /// Map x coordinate in tiles.
    pub x: u32,
    /// Map y coordinate in tiles.
    pub y: u32,
    /// Event pages in file order; page 0 is the default page.
    pub pages: Vec<MapEventPage>,
}

impl MapEvent {
    /// First byte of an event record.
    pub const MAGIC: u8 = Map::EVENT_MAGIC;

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        r.expect_u8(Self::MAGIC, "map event magic")?;
        let magic2 = r.u32()?;
        if magic2 != Map::EVENT_MAGIC2 {
            return Err(Error::field(
                "map event magic2",
                r.pos() - 4,
                format!("expected {}, got {magic2}", Map::EVENT_MAGIC2),
            ));
        }
        let id = r.u32()?;
        let name = r.t_str()?;
        let x = r.u32()?;
        let y = r.u32()?;
        let page_count = r.u32()? as usize;
        let separator = r.u32()?;
        if separator != Map::EVENT_SEPARATOR {
            return Err(Error::field(
                "map event separator",
                r.pos() - 4,
                format!("expected {}, got {separator}", Map::EVENT_SEPARATOR),
            ));
        }
        // A page record is at least 72 bytes, so this loose bound rejects a
        // corrupt count without risking a false rejection.
        if page_count > r.remaining() / 8 {
            return Err(Error::field(
                "map event page count",
                r.pos() - 8,
                format!("{page_count} pages do not fit in {} bytes", r.remaining()),
            ));
        }
        let mut pages = Vec::with_capacity(page_count);
        for _ in 0..page_count {
            pages.push(MapEventPage::read(r)?);
        }
        r.expect_u8(Map::EVENT_FOOTER, "map event footer")?;
        Ok(Self {
            id,
            name,
            x,
            y,
            pages,
        })
    }
}

/// One page of a map event.
///
/// The condition block is kept raw on purpose: each `trigger_switches` byte packs
/// an operator in the high nibble and an "enabled" flag in the low nibble. Across
/// the 134 shipped pages the 536 bytes take exactly the values `0x10`, `0x11`,
/// `0x20`, `0x21`, `0x31`, `0x50`, `0x51` (see docs/FORMAT_SPEC.md §5.6), so the
/// low nibble is a flag and the high nibble is a small operator id — the spec
/// documents both halves but never as a closed range, hence raw storage.
#[derive(Debug, Clone)]
pub struct MapEventPage {
    /// Graphic chip id, `0xFFFFFFFF` when the graphic comes from [`Self::graphic`].
    pub graphic_chip_id: u32,
    /// Graphic file name relative to the `Picture` folder.
    pub graphic: String,
    /// Facing direction, 0-7.
    pub direction: u8,
    /// Animation frame, 0-2.
    pub frame: u8,
    /// Opacity, 0-255.
    pub opacity: u8,
    /// Render mode: 0 normal, 1 multiply, 2 add, 3 subtract.
    pub display_type: u8,
    /// Trigger condition: 0 confirm, 1 autorun, 2 parallel, 3 player-touch,
    /// 4 event-touch. Only 0-3 occur in this game.
    pub trigger: u8,
    /// Raw trigger switch bytes: high nibble operator, low nibble enabled.
    pub trigger_switches: [u8; 4],
    /// Switch / variable numbers referenced by the condition block.
    pub trigger_variables: [u32; 4],
    /// Comparison values for [`Self::trigger_variables`].
    pub trigger_values: [i32; 4],
    /// Page movement settings and custom move route.
    pub route: RouteData,
    /// Event commands executed by this page.
    pub commands: Vec<Slot>,
    /// Feature word; always 3 in this game (bit 0 movement, bit 1 move route).
    pub features: u32,
    /// Shadow graphic id.
    pub shadow_graphic: u8,
    /// Collision range extension in x, 0-250.
    pub collision_width: u8,
    /// Collision range extension in y, 0-250.
    pub collision_height: u8,
    /// Raw page terminator: `0x7A` normally, `0x00` for the two tolerated pages.
    pub footer: u8,
}

impl MapEventPage {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        r.expect_u8(Map::PAGE_MAGIC, "map page magic")?;
        let graphic_chip_id = r.u32()?;
        let graphic = r.t_str()?;
        let direction = r.u8()?;
        let frame = r.u8()?;
        let opacity = r.u8()?;
        let display_type = r.u8()?;
        // 37-byte condition block, see docs/FORMAT_SPEC.md §5.6.1.
        let trigger = r.u8()?;
        let mut trigger_switches = [0u8; 4];
        for byte in &mut trigger_switches {
            *byte = r.u8()?;
        }
        let mut trigger_variables = [0u32; 4];
        for value in &mut trigger_variables {
            *value = r.u32()?;
        }
        let mut trigger_values = [0i32; 4];
        for value in &mut trigger_values {
            *value = r.i32()?;
        }
        let route = RouteData::read(r)?;
        let command_count = r.u32()? as usize;
        // A blank slot is three bytes (param count, branch depth, string count),
        // so this bound can never reject a well-formed page.
        if command_count > r.remaining() / 3 {
            return Err(Error::field(
                "map page command count",
                r.pos() - 4,
                format!(
                    "{command_count} commands do not fit in {} bytes",
                    r.remaining()
                ),
            ));
        }
        let mut commands = Vec::with_capacity(command_count);
        for _ in 0..command_count {
            commands.push(Slot::read(r)?);
        }
        let features = r.u32()?;
        let shadow_graphic = r.u8()?;
        let collision_width = r.u8()?;
        let collision_height = r.u8()?;
        if features > 3 {
            // Bit 2 ("page transfer") adds one trailing byte. It never occurs in
            // this game's maps, but skipping it keeps the page walk in sync.
            r.skip(1)?;
        }
        let footer = r.u8()?;
        if footer != Map::PAGE_FOOTER {
            let tolerated = footer == Map::PAGE_FOOTER_TOLERATED
                && (r.at_end() || matches!(r.peek_u8()?, Map::EVENT_FOOTER | Map::PAGE_MAGIC));
            if !tolerated {
                return Err(Error::magic(
                    "map page footer",
                    r.pos() - 1,
                    &[footer],
                    &[Map::PAGE_FOOTER],
                ));
            }
        }
        Ok(Self {
            graphic_chip_id,
            graphic,
            direction,
            frame,
            opacity,
            display_type,
            trigger,
            trigger_switches,
            trigger_variables,
            trigger_values,
            route,
            commands,
            features,
            shadow_graphic,
            collision_width,
            collision_height,
            footer,
        })
    }
}

/// Decoded meaning of a raw tile word, see docs/FORMAT_SPEC.md §5.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    /// No tile on this layer at this position (`0xFFFFFFFF`).
    Empty,
    /// An autotile: `id` selects the autotile within the tileset and `corners`
    /// holds the four quadrant variants in `[top_left, top_right, bottom_left,
    /// bottom_right]` order, each 0-9.
    Autotile { id: u32, corners: [u8; 4] },
    /// A plain base tile; `id` indexes the tileset's base tile image.
    Base(u32),
}

/// Decode a raw tile word into an autotile reference or a base tile id.
///
/// The encoding is decimal: `autotile_id = raw / 100000`, and when that is
/// non-zero the remaining four decimal digits are the four quadrant variants.
/// `0xFFFFFFFF` is the only non-decimal value and means "no tile".
pub fn decode_tile(raw: u32) -> Tile {
    if raw == Map::EMPTY_TILE {
        return Tile::Empty;
    }
    let autotile_id = raw / 100_000;
    if autotile_id > 0 {
        Tile::Autotile {
            id: autotile_id,
            corners: [
                ((raw % 10_000) / 1_000) as u8,
                ((raw % 1_000) / 100) as u8,
                ((raw % 100) / 10) as u8,
                (raw % 10) as u8,
            ],
        }
    } else {
        Tile::Base(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_is_ten_zero_bytes_then_wolfm() {
        assert_eq!(&Map::MAGIC[..10], &[0u8; 10]);
        assert_eq!(&Map::MAGIC[10..], &[0x57, 0x4F, 0x4C, 0x46, 0x4D, 0x00]);
        assert_eq!(Map::MAGIC.len(), 16);
    }

    #[test]
    fn decode_tile_splits_autotile_digits() {
        // Measured first tile of SampleMapA layer 0: autotile 1, four 4s.
        assert_eq!(
            decode_tile(104_444),
            Tile::Autotile {
                id: 1,
                corners: [4, 4, 4, 4]
            }
        );
        assert_eq!(
            decode_tile(100_000),
            Tile::Autotile {
                id: 1,
                corners: [0, 0, 0, 0]
            }
        );
        assert_eq!(decode_tile(1), Tile::Base(1));
        assert_eq!(decode_tile(292), Tile::Base(292));
        assert_eq!(decode_tile(99_999), Tile::Base(99_999));
        assert_eq!(decode_tile(Map::EMPTY_TILE), Tile::Empty);
    }

    #[test]
    fn layout_constants_match_the_spec() {
        assert_eq!(LAYER_COUNT, 3);
        assert_eq!(Map::EVENT_MAGIC, 0x6F);
        assert_eq!(Map::EVENT_MAGIC2, 0x3039);
        assert_eq!(Map::EVENT_FOOTER, 0x70);
        assert_eq!(Map::PAGE_MAGIC, 0x79);
        assert_eq!(Map::PAGE_FOOTER, 0x7A);
        assert_eq!(Map::FOOTER, 0x66);
        assert_eq!(Map::UNKNOWN1, 100);
    }
}
