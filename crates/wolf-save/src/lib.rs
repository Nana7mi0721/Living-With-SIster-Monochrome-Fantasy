//! WOLF RPG Editor save file (.sav) decoding.
//!
//! The on-disk layout is documented in `docs/SAVE_SPEC.md`. It was recovered by
//! reading the vendored reference implementation at `docs/vendor/WolfSave/`
//! (MIT) line by line and then checking every field against all nine saves this
//! game ships, so both agree byte for byte.
//!
//! A save file is **not** a chunk table. Everything below offset 0x14 is a
//! plaintext header; after it the body is one fixed-order stream:
//!
//! ```text
//! SavePart1 -> SavePart2 -> SavePart3 -> SavePart4 -> SavePart5
//!           -> VariableDatabase -> SavePart7 -> 0x19
//! ```
//!
//! No chunk ids exist on disk, so [`SaveFile::chunk`] hands out pseudo chunks whose id
//! is simply the segment order (`0..=6`). The byte ranges are real, which is all a
//! reader needs — see `docs/SAVE_SPEC.md` §1.3 for why `docs/FORMAT_SPEC.md`
//! §9.3's "chunk sequence" wording is wrong.
#![forbid(unsafe_code)]

pub mod part1;
pub mod part2;
pub mod part3;
pub mod part4;
pub mod part5;
pub mod part7;
pub mod vardb;

use wolf_format::bytes::Reader;
use wolf_format::error::{Error, Result};

pub use part1::{SavePart1, SavePart1_1};
pub use part2::SavePart2;
pub use part3::SavePart3;
pub use part4::SavePart4;
pub use part5::SavePart5;
pub use part7::SavePart7;
pub use vardb::VariableDatabase;

/// Offset the encrypted region starts at. Everything below it is plaintext.
pub const CRYPT_START: usize = 0x14;

/// Length of the plaintext header.
pub const HEADER_LEN: usize = 0x14;

/// Byte at [`CRYPT_START`] in a valid save (WolfSave calls it `m_var1`).
pub const START_BYTE: u8 = 0x19;

/// Byte at the very end of a valid save (`m_endByte`).
pub const END_BYTE: u8 = 0x19;

/// Synthetic chunk id of the global settings snapshot ([SavePart2]).
pub const PART2_ID: u32 = 1;

/// Synthetic chunk id of the map/event runtime tables ([SavePart3]).
pub const PART3_ID: u32 = 2;

/// Offset of `SavePart2::vars3_21` inside the SavePart2 segment.
///
/// `m_var1` and `m_var2` are single bytes, so the nineteen words start at 2.
/// Word 0 is the play time in frames.
pub const PART2_VARS3_OFFSET: usize = 2;

/// Highest `file_version` whose layout this crate implements.
///
/// WolfSave refuses anything above 0x8E. This game writes 0x8C, so the 0x8D and
/// 0x8E branches are implemented from the source but never exercised here.
pub const HIGHEST_SUPPORTED_VERSION: u16 = 0x8E;

/// Decrypt a save file, returning a fresh buffer.
///
/// The cipher is WOLF's usual one: three passes of an MSVC-CRT `rand()` stream XORed
/// into the body, with the seeds stored as plaintext at offsets 0, 3 and 9 and
/// the passes striding 1, 2 and 5 bytes.
///
/// Only the low three bits of each byte are touched (`rand() >> 12` keeps 3 bits), so
/// the top five bits are always plaintext — which is why the file is still
/// readable with `strings`.
pub fn decrypt(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() <= CRYPT_START {
        return Err(Error::field(
            "save file",
            0,
            format!(
                "only {} byte(s); a save file needs at least {}",
                data.len(),
                CRYPT_START + 1
            ),
        ));
    }
    let mut buf = data.to_vec();
    let seeds = [buf[0], buf[3], buf[9]];
    for (pass, step) in [1usize, 2, 5].into_iter().enumerate() {
        let mut state = u32::from(seeds[pass]);
        let mut at = CRYPT_START;
        while at < buf.len() {
            state = state.wrapping_mul(214_013).wrapping_add(2_531_011);
            let rand = (state >> 16) & 0x7FFF;
            buf[at] ^= ((rand >> 12) & 0xFF) as u8;
            at += step;
        }
    }
    Ok(buf)
}

/// Encrypt a decrypted save file, returning a fresh buffer.
///
/// This is the same three-pass XOR as [`decrypt`]: every byte below
/// [`CRYPT_START`] is copied through untouched (the seeds live there), and each
/// body byte is XORed once per pass with a keystream that depends only on its
/// own address, never on the data. The cipher is therefore an involution --
/// `encrypt(decrypt(x)) == x` and `decrypt(encrypt(x)) == x` -- which is why
/// this is a thin wrapper rather than a second implementation.
///
/// `docs/SAVE_SPEC.md` §2 records the byte-level evidence.
pub fn encrypt(data: &[u8]) -> Result<Vec<u8>> {
    decrypt(data)
}

/// The 0x14-byte plaintext header.
///
/// Offsets 1, 4, 5, 7, 8 and 0x0A..=0x13 are unexplained; they are all zero in
/// this game's saves. Keep the raw bytes rather than inventing names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveHeader {
    /// The header bytes exactly as they appear on disk (never encrypted).
    pub raw: [u8; HEADER_LEN],
    /// Byte at `0x14`, always [`START_BYTE`] here.
    pub start_byte: u8,
}

impl SaveHeader {
    /// First PRNG seed, stored at offset 0.
    #[must_use]
    pub fn seed0(&self) -> u8 {
        self.raw[0]
    }

    /// Byte sum of the decrypted body, stored at offset 2.
    #[must_use]
    pub fn checksum(&self) -> u8 {
        self.raw[2]
    }

    /// Second PRNG seed, stored at offset 3.
    #[must_use]
    pub fn seed1(&self) -> u8 {
        self.raw[3]
    }

    /// `0x55` at offset 6 means the strings are UTF-8 rather than Shift-JIS.
    #[must_use]
    pub fn is_utf8(&self) -> bool {
        self.raw[6] == 0x55
    }

    /// Third PRNG seed, stored at offset 9.
    #[must_use]
    pub fn seed2(&self) -> u8 {
        self.raw[9]
    }
}

/// One body segment, addressed by its position in the fixed stream.
///
/// The ids are synthetic — the file carries no chunk table — but the order is
/// stable across every save this game writes, which is enough to address the
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveChunk {
    /// Segment order, `0..=6`.
    pub id: u32,
    /// Segment name, matching the C++ class in `docs/vendor/WolfSave/`.
    pub name: &'static str,
    /// Absolute offset inside the *decrypted* buffer.
    pub offset: usize,
    /// Segment length in bytes.
    pub len: usize,
}

/// A fully parsed save file.
///
/// Every segment is typed; the decrypted buffer is kept so callers can still
/// reach the raw bytes of any segment.
#[derive(Debug, Clone)]
pub struct SaveFile {
    /// Plaintext header.
    pub header: SaveHeader,
    /// Save name (`m_name`), trailing NUL stripped.
    pub name: String,
    /// Format version (`m_fileVersion`); 0x8C here.
    pub file_version: u16,
    /// Whether strings are UTF-8 (from `header.raw[6] == 0x55`).
    pub is_utf8: bool,
    /// Map snapshot and object tables.
    pub part1: SavePart1,
    /// Global settings snapshot.
    pub part2: SavePart2,
    /// Map/event runtime tables.
    pub part3: SavePart3,
    /// The object-table continuation.
    pub part4: SavePart4,
    /// Per-common-event runtime state.
    pub part5: SavePart5,
    /// The variable database: every type's field config and every row.
    pub vardb: VariableDatabase,
    /// Trailing segment; 1 byte in every save this game writes.
    pub part7: SavePart7,
    /// Trailing byte, always [`END_BYTE`].
    pub end_byte: u8,
    /// Checksum byte as stored in the header.
    pub stored_checksum: u8,
    /// Checksum recomputed from the decrypted body.
    pub computed_checksum: u8,
    chunks: Vec<SaveChunk>,
    data: Vec<u8>,
}

impl SaveFile {
    /// Decrypt and parse a save file.
    pub fn parse(data: &[u8]) -> Result<SaveFile> {
        let buf = decrypt(data)?;
        let mut r = Reader::new(&buf);

        let mut raw = [0u8; HEADER_LEN];
        raw.copy_from_slice(r.take(HEADER_LEN)?);
        let is_utf8 = raw[6] == 0x55;

        let start_byte = r.u8()?;
        if start_byte != START_BYTE {
            return Err(Error::magic(
                "save start byte",
                CRYPT_START,
                &[start_byte],
                &[START_BYTE],
            ));
        }

        let name_raw = mem_data(&mut r, 2)?;
        let name = String::from_utf8_lossy(strip_nul(&name_raw)).into_owned();

        let fv_pos = r.pos();
        let file_version = r.u16()?;
        if file_version > HIGHEST_SUPPORTED_VERSION {
            return Err(Error::field(
                "file_version",
                fv_pos,
                format!(
                    "{file_version:#06X} is above the highest supported {HIGHEST_SUPPORTED_VERSION:#06X}"
                ),
            ));
        }

        let mut chunks = Vec::with_capacity(7);

        let offset = r.pos();
        let part1 = part1::parse(&mut r, file_version)?;
        chunks.push(SaveChunk {
            id: 0,
            name: "SavePart1",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let part2 = part2::parse(&mut r, file_version)?;
        chunks.push(SaveChunk {
            id: 1,
            name: "SavePart2",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let part3 = part3::parse(&mut r, file_version)?;
        chunks.push(SaveChunk {
            id: 2,
            name: "SavePart3",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let part4 = part4::parse(&mut r, file_version)?;
        chunks.push(SaveChunk {
            id: 3,
            name: "SavePart4",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let part5 = part5::parse(&mut r, file_version)?;
        chunks.push(SaveChunk {
            id: 4,
            name: "SavePart5",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let vardb = vardb::parse(&mut r)?;
        chunks.push(SaveChunk {
            id: 5,
            name: "VariableDatabase",
            offset,
            len: r.pos() - offset,
        });

        let offset = r.pos();
        let part7 = part7::parse(&mut r)?;
        chunks.push(SaveChunk {
            id: 6,
            name: "SavePart7",
            offset,
            len: r.pos() - offset,
        });

        if r.pos() + 1 != r.len() {
            return Err(Error::field(
                "save body",
                r.pos(),
                format!(
                    "the body ended at offset {} but the file is {} byte(s) long; exactly one trailing byte is expected",
                    r.pos(),
                    r.len()
                ),
            ));
        }
        let end_pos = r.pos();
        let end_byte = r.u8()?;
        if end_byte != END_BYTE {
            return Err(Error::magic(
                "save end byte",
                end_pos,
                &[end_byte],
                &[END_BYTE],
            ));
        }

        let computed_checksum = buf[CRYPT_START..]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));

        Ok(SaveFile {
            header: SaveHeader { raw, start_byte },
            name,
            file_version,
            is_utf8,
            part1,
            part2,
            part3,
            part4,
            part5,
            vardb,
            part7,
            end_byte,
            stored_checksum: raw[2],
            computed_checksum,
            chunks,
            data: buf,
        })
    }

    /// Build a structurally valid save file containing no game state.
    ///
    /// This is the "new game" template: a data root that ships no `Save/System.sav`
    /// (this repository's `work/all`, for example) still has to be able to write its
    /// first save, and [`SaveFile::rebuild`] needs something to rebuild Part1 and
    /// Part4 from.
    ///
    /// Everything is at its version-gate default for `file_version = 0x8C`:
    ///
    /// * the header marks the strings UTF-8 (`raw[6] = 0x55`) and the checksum is
    ///   computed from the synthesized body;
    /// * SavePart1 carries **no map snapshot** (the `0xFFFFFFFF` sentinel), 31 empty
    ///   automatic-tile names and the 11-word tail the version requires;
    /// * SavePart2 has every `>= 0x8C` block present but empty/zero;
    /// * SavePart3 has row 0 = the 800 normal variables (all zero) and 550 event
    ///   groups of 100 self numbers / 5 self strings, i.e. the shape a save this
    ///   game writes has; the other group/row/mds tables are empty;
    /// * SavePart4 has an all-default lead object and no further objects;
    /// * SavePart5 has no pictures, the variable database has no types, and SavePart7
    ///   is the one-byte `0` gate every retail save carries.
    ///
    /// The typed views and the stored bytes agree by construction, so
    /// [`SaveFile::rebuild`] and [`SaveFile::serialize`] produce the same bytes and
    /// [`SaveFile::to_bytes_rebuilt`] parses back to an equal file. No game data is
    /// read: 550 groups is the widest event table the retail saves use (500 is the
    /// narrowest), so every common event of this build has a self-variable slot.
    ///
    /// `new_game_template()` cannot fail: the defaults are chosen to satisfy each
    /// segment encoder's count checks. It replaces the old "no template save" refusal
    /// in `Game::save_to` when `Save/System.sav` is absent.
    #[must_use]
    pub fn new_game_template() -> SaveFile {
        const FILE_VERSION: u16 = 0x8C;
        // SavePart3 stores self numbers for common events; retail uses 500 or 550.
        const EVENT_GROUPS: u32 = 550;
        // Every event group holds CSelf0..CSelf99 in Part3.
        const SELF_NUMBERS: u8 = 100;
        // and CSelf5..CSelf9 as five MemData bodies.
        const SELF_STRINGS: u8 = 5;
        // Row 0 of Part3 is the 800 normal variables (docs/SAVE_SPEC.md §6.1).
        const NORMAL_VARS: u32 = 800;

        let name = "New Game".to_owned();
        let mut header = SaveHeader {
            raw: [0u8; HEADER_LEN],
            start_byte: START_BYTE,
        };
        // 0x55 at offset 6 is the "strings are UTF-8" marker (SAVE_SPEC §3).
        header.raw[6] = 0x55;

        let part1 = SavePart1 {
            // The three leading words are 1/0/0 in every save this game writes.
            var1: 1,
            var2: 0,
            var3: 0,
            // The dimensions are only meaningful with a snapshot; keep the retail
            // numbers so a re-encoded header still describes the shipping map grid.
            map_width: 64,
            map_height: 36,
            // An empty vector makes to_bytes emit the "no map snapshot" sentinel.
            map_tiles: Vec::new(),
            chip_image: Vec::new(),
            auto_tiles: vec![Vec::new(); 31],
            // The fixed 192-word switch table every retail save carries.
            vars2_new: vec![0; 192],
            vars2_old: Vec::new(),
            objects: Vec::new(),
            // measured constant in all nine retail saves (SAVE_SPEC §4.6).
            tail_vars: vec![0, 0, 0, 0, 100, 100, 0, 0, 0, 0, 100],
            vars3: Vec::new(),
        };

        let part2 = part2::SavePart2 {
            var1: 0,
            var2: 0,
            vars3_21: [0; 19],
            vars22_24: [0; 3],
            var25: 0,
            vars26_28: [0; 3],
            vars29_37: [0; 9],
            var38: 0,
            var39: 0,
            vars40_41: [0; 2],
            vars42_45: [0; 4],
            vars46_48: [0; 3],
            var49: 0,
            // m_var50 is only on disk below 0x60, so it stays a Rust-side zero.
            var50: 0,
            var51: 0,
            vars52_53: [0; 2],
            block100: Some(part2::SavePart2Block100 {
                var54: 0,
                var55: 0,
                var56: 0,
                vars1: Vec::new(),
                vars2: Vec::new(),
                vars59_63: [0; 5],
            }),
            vars64_69: [0; 6],
            var70: 0,
            md1_md2: Some((Vec::new(), Vec::new())),
            var71: 0,
            vars72_74: [0; 3],
            bgm: Some(part2::SavePart2Bgm {
                path1: Vec::new(),
                vars75_77: [0; 3],
                path2: Vec::new(),
                vars78_80: [0; 3],
            }),
            block109: Some(part2::SavePart2Block109 {
                background: Vec::new(),
                md6: Vec::new(),
                var81: 0,
            }),
            var82: 0,
            mds7_9: Some([Vec::new(), Vec::new(), Vec::new()]),
            var83: 0,
            var84: 0,
            var85: 0,
            var86: 0,
            var87: 0,
            vars88_89: [0; 2],
            var90: 0,
            var91: 0,
            vars92_95: [0; 4],
            var96: 0,
            var97: 0,
            block137: Some(part2::SavePart2Block137 {
                vars98_101: [0; 4],
                vars2: [0; 24],
            }),
            block8a: Some(part2::SavePart2Block8A {
                vars102_105: [0; 4],
                md10: Vec::new(),
                mds2: Vec::new(),
            }),
        };

        let part3 = SavePart3 {
            var1: 0,
            // No group table: the layer is unexplained in the reference source.
            var2: 0,
            group_counts: Vec::new(),
            group_bytes: Vec::new(),
            group_words: Vec::new(),
            // One row, row 0: the 800 normal variables.
            var3: 1,
            row_counts: vec![NORMAL_VARS],
            row_words: vec![0; NORMAL_VARS as usize],
            // No chapter/thumbnail strings from the save menu.
            var4: 0,
            mds1: Vec::new(),
            var5: EVENT_GROUPS,
            list_counts: vec![SELF_NUMBERS; EVENT_GROUPS as usize],
            list_words: vec![0; EVENT_GROUPS as usize * usize::from(SELF_NUMBERS)],
            // The encoder rejects var6 == 0 (parse treats it as corruption), so the
            // template fills the whole event table with empty slots instead.
            var6: EVENT_GROUPS,
            slot_counts: vec![SELF_STRINGS; EVENT_GROUPS as usize],
            slot_strings: vec![vec![0u8]; EVENT_GROUPS as usize * usize::from(SELF_STRINGS)],
        };

        let part4 = SavePart4 {
            lead: empty_object(),
            objects: Vec::new(),
            var2: 0,
            var3: 0,
            vars2: Vec::new(),
            vars3: Vec::new(),
        };

        let part5 = SavePart5 {
            // Bit 15 clear with a zero count is the ordinary "no pictures" table.
            var1: 0,
            events: Vec::new(),
        };

        let vardb = VariableDatabase {
            // The byte every retail database stores; no types means no rows.
            unknown: 0x6F,
            type_count: 0,
            types: Vec::new(),
        };

        let part7 = SavePart7 {
            // Anything but 1 ends SavePart7 immediately: this is its whole body.
            var1: 0,
            entries: Vec::new(),
        };

        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);
        let name_len = u16::try_from(name_bytes.len())
            .expect("the template name is far shorter than a MemData<WORD>");

        let mut data = Vec::new();
        data.extend_from_slice(&header.raw);
        data.push(header.start_byte);
        data.extend_from_slice(&name_len.to_le_bytes());
        data.extend_from_slice(&name_bytes);
        data.extend_from_slice(&FILE_VERSION.to_le_bytes());

        let mut chunks = Vec::with_capacity(7);
        let segments = [
            ("SavePart1", part1.to_bytes(FILE_VERSION)),
            ("SavePart2", part2.to_bytes(FILE_VERSION)),
            ("SavePart3", part3.to_bytes_with_version(FILE_VERSION)),
            ("SavePart4", part4.to_bytes(FILE_VERSION)),
            ("SavePart5", part5.to_bytes(FILE_VERSION)),
            ("VariableDatabase", vardb.to_bytes()),
        ];
        for (id, (segment, encoded)) in segments.into_iter().enumerate() {
            let encoded = encoded.expect("the synthesized new-game segment is consistent");
            let offset = data.len();
            data.extend_from_slice(&encoded);
            chunks.push(SaveChunk {
                id: id as u32,
                name: segment,
                offset,
                len: encoded.len(),
            });
        }
        // SavePart7 has no writer yet; its default body is exactly the gate byte.
        let offset = data.len();
        data.push(part7.var1);
        chunks.push(SaveChunk {
            id: PART7_ID,
            name: "SavePart7",
            offset,
            len: 1,
        });
        data.push(END_BYTE);

        let checksum = data[CRYPT_START..]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        data[2] = checksum;
        header.raw[2] = checksum;

        SaveFile {
            header,
            name,
            file_version: FILE_VERSION,
            is_utf8: true,
            part1,
            part2,
            part3,
            part4,
            part5,
            vardb,
            part7,
            end_byte: END_BYTE,
            stored_checksum: checksum,
            computed_checksum: checksum,
            chunks,
            data,
        }
    }

    /// Every segment, in file order.
    #[must_use]
    pub fn chunks(&self) -> &[SaveChunk] {
        &self.chunks
    }

    /// Look a segment up by its synthetic id (`0..=6`).
    #[must_use]
    pub fn chunk(&self, id: u32) -> Option<&SaveChunk> {
        self.chunks.iter().find(|chunk| chunk.id == id)
    }

    /// The decrypted bytes of a segment.
    #[must_use]
    pub fn chunk_bytes(&self, id: u32) -> Option<&[u8]> {
        self.chunk(id)
            .map(|chunk| &self.data[chunk.offset..chunk.offset + chunk.len])
    }

    /// The whole decrypted file.
    #[must_use]
    pub fn decrypted(&self) -> &[u8] {
        &self.data
    }

    /// Whether the stored checksum matches the body.
    #[must_use]
    pub fn checksum_ok(&self) -> bool {
        self.stored_checksum == self.computed_checksum
    }

    /// Rebuild the **decrypted** file from the header and the current segments.
    ///
    /// The plaintext header is copied from [`SaveFile::header`] except for the
    /// checksum byte at offset 2, which is recomputed from the new body. The name
    /// is re-encoded as `MemData<WORD>` (byte length including the terminator) and
    /// the version as a little-endian `u16`, so a caller can edit
    /// [`SaveFile::name`], [`SaveFile::file_version`], [`SaveFile::header`] or any
    /// segment and still get a self-consistent file.
    ///
    /// Unlike a parsed segment, the result is *not* encrypted — feed it to
    /// [`SaveFile::to_bytes`] to write a file the game can load.
    ///
    /// The typed views are not refreshed; they still describe the bytes that were
    /// parsed. Re-run [`SaveFile::parse`] on the output to get matching views.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut name_bytes = self.name.as_bytes().to_vec();
        name_bytes.push(0);
        let name_len = match u16::try_from(name_bytes.len()) {
            Ok(len) => len,
            Err(_) => {
                return Err(Error::field(
                    "save name",
                    0,
                    format!(
                        "{} byte(s) including the terminator; a MemData<WORD> holds at most 65535",
                        name_bytes.len()
                    ),
                ));
            }
        };

        let body_len: usize = self.chunks.iter().map(|chunk| chunk.len).sum();
        let mut out = Vec::with_capacity(CRYPT_START + 1 + 2 + name_bytes.len() + 2 + body_len + 1);
        out.extend_from_slice(&self.header.raw);
        out.push(self.header.start_byte);
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&name_bytes);
        out.extend_from_slice(&self.file_version.to_le_bytes());
        for chunk in &self.chunks {
            let end = chunk.offset + chunk.len;
            if end > self.data.len() {
                return Err(Error::field(
                    chunk.name,
                    chunk.offset,
                    format!(
                        "segment ends at {end} but only {} byte(s) are buffered",
                        self.data.len()
                    ),
                ));
            }
            out.extend_from_slice(&self.data[chunk.offset..end]);
        }
        out.push(self.end_byte);

        let checksum = out[CRYPT_START..]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        out[2] = checksum;
        Ok(out)
    }

    /// Encrypt [`SaveFile::serialize`]: the exact bytes to write to disk.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        encrypt(&self.serialize()?)
    }

    /// Replace one segment's bytes, keeping every later offset consistent.
    ///
    /// The ids are the ones [`SaveFile::chunks`] hands out (`0..=6`). The new segment
    /// may have a different length; segments after it are shifted and the checksum
    /// is recomputed. Nothing is written to disk — call [`SaveFile::to_bytes`].
    ///
    /// The typed views are **not** re-parsed, so they keep describing the previous
    /// bytes. Round-trip to refresh them:
    ///
    /// ```text
    /// let mut edited = save.clone();
    /// edited.replace_part(6, &[1, 1, 0, 0, 0, 0x05, 0x2A])?;  // one SavePart7 entry
    /// let fresh = SaveFile::parse(&edited.to_bytes()?)?;
    /// assert_eq!(fresh.part7.entries.len(), 1);
    /// ```
    pub fn replace_part(&mut self, id: u32, bytes: &[u8]) -> Result<()> {
        let index = match self.chunks.iter().position(|chunk| chunk.id == id) {
            Some(index) => index,
            None => {
                return Err(Error::field(
                    "segment id",
                    0,
                    format!(
                        "no segment with id {id}; this file has ids 0..={}",
                        self.chunks.len().saturating_sub(1)
                    ),
                ));
            }
        };

        let offset = self.chunks[index].offset;
        let old_len = self.chunks[index].len;
        self.data
            .splice(offset..offset + old_len, bytes.iter().copied());
        self.chunks[index].len = bytes.len();

        let delta = bytes.len() as i64 - old_len as i64;
        for later in &mut self.chunks[index + 1..] {
            later.offset = (later.offset as i64 + delta).max(0) as usize;
        }

        self.computed_checksum = self.data[CRYPT_START..]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        Ok(())
    }

    /// Replace the variable tables ([SavePart3]) and refresh the typed view.
    ///
    /// The segment is re-encoded with this file's [SaveFile::file_version], so
    /// a variable edit costs one segment rewrite and nothing else moves
    /// semantically -- only the offsets of the later segments shift.
    pub fn set_part3(&mut self, part: &SavePart3) -> Result<()> {
        let bytes = part.to_bytes_with_version(self.file_version)?;
        self.replace_part(PART3_ID, &bytes)?;
        self.part3 = part.clone();
        Ok(())
    }

    /// Play time in frames, the value the game shows on the load screen.
    ///
    /// This is m_var3 of [SavePart2] (word 0 of its vars3_21 array), which sits
    /// at offset 2 inside the segment: var1 and var2 are single bytes. The game
    /// runs at 60 fps, so 16705 frames is 4:38.
    #[must_use]
    pub fn playtime_frames(&self) -> Option<u32> {
        self.part2.vars3_21.first().copied()
    }

    /// Overwrite the play time in frames; see [SaveFile::playtime_frames].
    pub fn set_playtime_frames(&mut self, frames: u32) -> Result<()> {
        let mut bytes = match self.chunk_bytes(PART2_ID) {
            Some(bytes) => bytes.to_vec(),
            None => {
                return Err(Error::field(
                    "SavePart2",
                    0,
                    "this file has no second segment to patch".to_string(),
                ))
            }
        };
        let at = PART2_VARS3_OFFSET;
        if bytes.len() < at + 4 {
            return Err(Error::field(
                "SavePart2",
                at,
                format!(
                    "only {} byte(s); the play-time word ends at {}",
                    bytes.len(),
                    at + 4
                ),
            ));
        }
        bytes[at..at + 4].copy_from_slice(&frames.to_le_bytes());
        self.replace_part(PART2_ID, &bytes)?;
        self.part2.vars3_21[0] = frames;
        Ok(())
    }

    /// Normal variable 2000000 + index; see [SavePart3::normal_var].
    #[must_use]
    pub fn normal_var(&self, index: usize) -> Option<i32> {
        self.part3.normal_var(index)
    }

    /// Overwrite normal variable 2000000 + index and re-encode the tables.
    ///
    /// Returns false (and changes nothing) when the index is out of range.
    pub fn set_normal_var(&mut self, index: usize, value: i32) -> Result<bool> {
        if !self.part3.set_normal_var(index, value) {
            return Ok(false);
        }
        let part = self.part3.clone();
        self.set_part3(&part)?;
        Ok(true)
    }

    /// Self variable slot of common event event; see [SavePart3::self_var].
    #[must_use]
    pub fn self_var(&self, event: u32, slot: usize) -> Option<i32> {
        self.part3.self_var(event, slot)
    }

    /// Overwrite one self variable and re-encode the tables.
    pub fn set_self_var(&mut self, event: u32, slot: usize, value: i32) -> Result<bool> {
        if !self.part3.set_self_var(event, slot, value) {
            return Ok(false);
        }
        let part = self.part3.clone();
        self.set_part3(&part)?;
        Ok(true)
    }

    /// Self string variable slot of common event event; see
    /// [SavePart3::self_string]. Note that slot 0 is CSelf5.
    #[must_use]
    pub fn self_string(&self, event: u32, slot: usize) -> Option<&str> {
        self.part3.self_string(event, slot)
    }

    /// Overwrite one self string variable and re-encode the tables.
    pub fn set_self_string(&mut self, event: u32, slot: usize, text: &str) -> Result<bool> {
        if !self.part3.set_self_string(event, slot, text) {
            return Ok(false);
        }
        let part = self.part3.clone();
        self.set_part3(&part)?;
        Ok(true)
    }
}

/// Synthetic chunk ids for the segments that have their own writer.
pub const PART1_ID: u32 = 0;
/// Synthetic chunk id of [SavePart4].
pub const PART4_ID: u32 = 3;
/// Synthetic chunk id of [SavePart5].
pub const PART5_ID: u32 = 4;
/// Synthetic chunk id of [VariableDatabase].
pub const VARDB_ID: u32 = 5;
/// Synthetic chunk id of [SavePart7] (no writer yet; copied verbatim).
pub const PART7_ID: u32 = 6;

// ---- little-endian writers shared by the segment encoders ---------------

pub(crate) fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

pub(crate) fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Append a signed word. The C++ writes `int32_t` and `uint32_t` with the same
/// `Write`, so the two helpers differ only in what the caller can hand them.
pub(crate) fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Append a count the way the C++ writes one: a little-endian u32.
pub(crate) fn push_count(out: &mut Vec<u8>, count: usize) -> Result<()> {
    match u32::try_from(count) {
        Ok(value) => {
            push_u32(out, value);
            Ok(())
        }
        Err(_) => Err(Error::field(
            "count",
            out.len(),
            format!("{count} item(s) do not fit in a u32 count"),
        )),
    }
}

/// Append one MemData body: a byte length of size bytes, then the bytes.
pub(crate) fn push_mem_data(out: &mut Vec<u8>, body: &[u8], size: u8) -> Result<()> {
    let too_big = || {
        Error::field(
            "MemData length",
            out.len(),
            format!("{} byte(s) do not fit in a {size}-byte length", body.len()),
        )
    };
    match size {
        1 => match u8::try_from(body.len()) {
            Ok(len) => out.push(len),
            Err(_) => return Err(too_big()),
        },
        2 => match u16::try_from(body.len()) {
            Ok(len) => push_u16(out, len),
            Err(_) => return Err(too_big()),
        },
        4 => match u32::try_from(body.len()) {
            Ok(len) => push_u32(out, len),
            Err(_) => return Err(too_big()),
        },
        other => {
            return Err(Error::field(
                "MemData element size",
                out.len(),
                format!("{other} is not 1, 2 or 4"),
            ))
        }
    }
    out.extend_from_slice(body);
    Ok(())
}

impl SaveFile {
    /// Rebuild the decrypted file from the **typed views**, not from the raw
    /// bytes captured at parse time.
    ///
    /// This is the writer-side counterpart of [SaveFile::serialize], which copies
    /// each segment's original bytes verbatim:
    ///
    /// * SavePart1 -> [SavePart1::to_bytes]
    /// * SavePart2 -> [SavePart2::to_bytes]
    /// * SavePart3 -> [SavePart3::to_bytes_with_version]
    /// * SavePart4 -> [SavePart4::to_bytes]
    /// * SavePart5 -> [SavePart5::to_bytes]
    /// * VariableDatabase -> [VariableDatabase::to_bytes]
    /// * SavePart7 -> copied verbatim (no writer yet)
    ///
    /// So an edit to any typed field -- including one cell of the variable
    /// database -- reaches the output; only SavePart7 still travels as its
    /// parsed bytes.
    ///
    /// The header is written from [SaveFile::header] with the checksum byte at
    /// offset 2 recomputed over the new body; the name is re-encoded as a
    /// length-prefixed MemData (byte length including the NUL) and the version
    /// as a little-endian u16. The result is **not** encrypted -- feed it to
    /// [SaveFile::to_bytes_rebuilt] to get something the game can load.
    pub fn rebuild(&self) -> Result<Vec<u8>> {
        let mut name_bytes = self.name.as_bytes().to_vec();
        name_bytes.push(0);
        let name_len = match u16::try_from(name_bytes.len()) {
            Ok(len) => len,
            Err(_) => {
                return Err(Error::field(
                    "save name",
                    0,
                    format!(
                        "{} byte(s) including the terminator; a MemData<WORD> holds at most 65535",
                        name_bytes.len()
                    ),
                ));
            }
        };

        let part1 = self.part1.to_bytes(self.file_version)?;
        let part2 = self.part2.to_bytes(self.file_version)?;
        let part3 = self.part3.to_bytes_with_version(self.file_version)?;
        let part4 = self.part4.to_bytes(self.file_version)?;
        let part5 = self.part5.to_bytes(self.file_version)?;
        let vardb = self.vardb.to_bytes()?;
        let part7 = match self.chunk_bytes(PART7_ID) {
            Some(bytes) => bytes.to_vec(),
            None => {
                return Err(Error::field(
                    "SavePart7",
                    0,
                    "this file has no seventh segment".to_string(),
                ))
            }
        };

        let body_len = part1.len()
            + part2.len()
            + part3.len()
            + part4.len()
            + part5.len()
            + vardb.len()
            + part7.len();
        let mut out = Vec::with_capacity(CRYPT_START + 1 + 2 + name_bytes.len() + 2 + body_len + 1);
        out.extend_from_slice(&self.header.raw);
        out.push(self.header.start_byte);
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&name_bytes);
        out.extend_from_slice(&self.file_version.to_le_bytes());
        out.extend_from_slice(&part1);
        out.extend_from_slice(&part2);
        out.extend_from_slice(&part3);
        out.extend_from_slice(&part4);
        out.extend_from_slice(&part5);
        out.extend_from_slice(&vardb);
        out.extend_from_slice(&part7);
        out.push(self.end_byte);

        let checksum = out[CRYPT_START..]
            .iter()
            .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        out[2] = checksum;
        Ok(out)
    }

    /// Encrypt [SaveFile::rebuild]: the exact bytes to write to disk.
    pub fn to_bytes_rebuilt(&self) -> Result<Vec<u8>> {
        encrypt(&self.rebuild()?)
    }

    /// Replace the map snapshot (SavePart1) and refresh the typed view.
    pub fn set_part1(&mut self, part: &SavePart1) -> Result<()> {
        let bytes = part.to_bytes(self.file_version)?;
        self.replace_part(PART1_ID, &bytes)?;
        self.part1 = part.clone();
        Ok(())
    }

    /// Replace the global settings (SavePart2) and refresh the typed view.
    ///
    /// This also refreshes the play-time view, which lives in this segment.
    pub fn set_part2(&mut self, part: &SavePart2) -> Result<()> {
        let bytes = part.to_bytes(self.file_version)?;
        self.replace_part(PART2_ID, &bytes)?;
        self.part2 = part.clone();
        Ok(())
    }

    /// Replace the object-table continuation (SavePart4) and refresh the view.
    pub fn set_part4(&mut self, part: &SavePart4) -> Result<()> {
        let bytes = part.to_bytes(self.file_version)?;
        self.replace_part(PART4_ID, &bytes)?;
        self.part4 = part.clone();
        Ok(())
    }

    /// Replace the per-event runtime state (SavePart5) and refresh the view.
    pub fn set_part5(&mut self, part: &SavePart5) -> Result<()> {
        let bytes = part.to_bytes(self.file_version)?;
        self.replace_part(PART5_ID, &bytes)?;
        self.part5 = part.clone();
        Ok(())
    }

    /// Replace the variable database and refresh the view.
    ///
    /// This is the writer the running game needs: opcode 250 edits
    /// [VariableDatabase] through the setters below, and one call here turns
    /// those cell edits into the sixth segment.
    pub fn set_vardb(&mut self, vardb: &VariableDatabase) -> Result<()> {
        let bytes = vardb.to_bytes()?;
        self.replace_part(VARDB_ID, &bytes)?;
        self.vardb = vardb.clone();
        Ok(())
    }

    /// Write one numeric cell of the variable database and re-encode the segment.
    ///
    /// Returns false when the type/row/field does not exist or the field is
    /// textual; nothing is written in that case.
    pub fn set_vardb_num(
        &mut self,
        type_index: usize,
        row_index: usize,
        field_index: usize,
        value: i32,
    ) -> Result<bool> {
        let mut vardb = self.vardb.clone();
        if !vardb.set_num(type_index, row_index, field_index, value) {
            return Ok(false);
        }
        self.set_vardb(&vardb)?;
        Ok(true)
    }

    /// Write one textual cell of the variable database and re-encode the segment.
    ///
    /// The body is stored verbatim, NUL included -- use [SaveFile::set_vardb_text]
    /// when the caller has a &str.
    pub fn set_vardb_string(
        &mut self,
        type_index: usize,
        row_index: usize,
        field_index: usize,
        body: &[u8],
    ) -> Result<bool> {
        let mut vardb = self.vardb.clone();
        if !vardb.set_string(type_index, row_index, field_index, body) {
            return Ok(false);
        }
        self.set_vardb(&vardb)?;
        Ok(true)
    }

    /// Write one textual cell from a &str, appending the NUL the game stores.
    pub fn set_vardb_text(
        &mut self,
        type_index: usize,
        row_index: usize,
        field_index: usize,
        text: &str,
    ) -> Result<bool> {
        let mut vardb = self.vardb.clone();
        if !vardb.set_text(type_index, row_index, field_index, text) {
            return Ok(false);
        }
        self.set_vardb(&vardb)?;
        Ok(true)
    }
}

/// A `SavePart1_1` (object) with every optional table empty and the two
/// retail "nothing here" sentinels set.
///
/// `SavePart1_1` has no `Default` derive (its fields are a straight
/// transcription of the C++ `m_var1..m_var39`), so the new-game template spells
/// the defaults out here. `var1 = 0xFFFFFFFE` is the value every retail SavePart4
/// lead object carries; `var6 = 0xFFFFFFFF` is what every retail object carries;
/// `md1` holds the single NUL byte a MemData<WORD> string slot has.
fn empty_object() -> SavePart1_1 {
    SavePart1_1 {
        var1: u32::MAX - 1,
        var2: 0,
        var3: 0,
        var4: 0,
        var5: 0,
        var6: u32::MAX,
        md1: vec![0],
        var7: 0,
        var8: 0,
        var9: 0,
        var10: 0,
        var11: 0,
        var12: 0,
        sp1: part1::SavePart1_1_1::default(),
        sp2: part1::SavePart1_1_1::default(),
        var13: 0,
        var14: 0,
        var15: 0,
        var16: 0,
        var17: 0,
        var18: 0,
        var19: 0,
        var20: 0,
        var21: 0,
        vars1: Vec::new(),
        var23: 0,
        var24: 0,
        var25: 0,
        var26: 0,
        var27: 0,
        var28: 0,
        var29: 0,
        var30: 0,
        var31: 0,
        var32: 0,
        mds1: Vec::new(),
        var34: 0,
        var35: 0,
        var36: 0,
        var37: 0,
        var38: 0,
        var39: 0,
    }
}

/// Read one `MemData<T>` body: a length whose width is `size`, then the bytes.
///
/// `MemData` stores a **byte** count, not an element count, and a count of zero
/// consumes nothing at all. WolfSave's `toString` returns an empty string for
/// anything of length 0 or 1, so callers that want text must strip the NUL.
pub(crate) fn mem_data(r: &mut Reader<'_>, size: u8) -> Result<Vec<u8>> {
    let pos = r.pos();
    let n = match size {
        1 => usize::from(r.u8()?),
        2 => usize::from(r.u16()?),
        4 => r.u32()? as usize,
        other => {
            return Err(Error::field(
                "MemData element size",
                pos,
                format!("{other} is not 1, 2 or 4"),
            ));
        }
    };
    if n > r.remaining() {
        return Err(Error::eof(pos, n, r.remaining()));
    }
    Ok(r.take(n)?.to_vec())
}

/// Drop a single trailing NUL, matching WolfSave's `MemData::toString`.
pub(crate) fn strip_nul(bytes: &[u8]) -> &[u8] {
    match bytes.last() {
        Some(0) => &bytes[..bytes.len() - 1],
        _ => bytes,
    }
}

/// A `MemData` body decoded as text the way the game displays it.
///
/// WolfSave returns an empty string for bodies of length 0 or 1, because those
/// hold at most a bare terminator.
#[must_use]
pub fn mem_string(bytes: &[u8]) -> String {
    if bytes.len() <= 1 {
        return String::new();
    }
    String::from_utf8_lossy(strip_nul(bytes)).into_owned()
}

/// Interpret a count field the way the C++ does: `if ((int)m_varX > 0)`.
///
/// The counters are stored as `DWORD` but compared as `int`, so 0xFFFFFFFF means
/// "none", not "four billion".
pub(crate) fn signed_count(value: u32) -> usize {
    let signed = value as i32;
    if signed > 0 {
        signed as usize
    } else {
        0
    }
}

/// Fail unless `count` items of `item` bytes fit in what is left.
///
/// The reference implementation happily trusts these counters; a corrupt save
/// would then allocate gigabytes. Every loop here is bounded by the bytes
/// actually present instead.
pub(crate) fn ensure_fits(
    r: &Reader<'_>,
    count: usize,
    item: usize,
    what: &'static str,
) -> Result<()> {
    match count.checked_mul(item) {
        Some(bytes) if bytes <= r.remaining() => Ok(()),
        _ => Err(Error::field(
            what,
            r.pos(),
            format!(
                "{count} item(s) of {item} byte(s) cannot fit in the {} remaining byte(s)",
                r.remaining()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_synthesized_new_game_template_round_trips() {
        let save = SaveFile::new_game_template();
        assert_eq!(save.file_version, 0x8C);
        assert_eq!(save.header.start_byte, START_BYTE);
        assert_eq!(save.end_byte, END_BYTE);
        assert!(save.header.is_utf8());
        assert!(save.checksum_ok());
        assert_eq!(save.header.raw[6], 0x55);

        // The typed views and the bytes captured by the constructor agree, so the two
        // writers produce the same file.
        let serialized = save.serialize().expect("serialize");
        let rebuilt = save.rebuild().expect("rebuild");
        assert_eq!(
            serialized, rebuilt,
            "the synthesized bytes and the typed views must agree"
        );

        let bytes = save
            .to_bytes_rebuilt()
            .expect("encrypt the rebuilt template");
        let parsed = SaveFile::parse(&bytes).expect("parse the synthesized template");
        assert!(parsed.checksum_ok());
        assert_eq!(parsed.file_version, save.file_version);
        assert_eq!(parsed.name, save.name);
        assert_eq!(parsed.part1, save.part1);
        assert_eq!(parsed.part2, save.part2);
        assert_eq!(parsed.part3, save.part3);
        assert_eq!(parsed.part4, save.part4);
        assert_eq!(parsed.part5, save.part5);
        assert_eq!(parsed.vardb, save.vardb);
        assert_eq!(parsed.part7, save.part7);
        assert_eq!(parsed.header.start_byte, START_BYTE);
        assert_eq!(parsed.end_byte, END_BYTE);

        assert_eq!(parsed.to_bytes().expect("to_bytes"), bytes);
        assert_eq!(parsed.to_bytes_rebuilt().expect("to_bytes_rebuilt"), bytes);
    }

    #[test]
    fn the_synthesized_template_has_the_documented_defaults() {
        let save = SaveFile::new_game_template();
        assert!(
            save.part1.map_tiles.is_empty(),
            "an empty map_tiles vector is the no-snapshot sentinel"
        );
        assert_eq!(save.part1.auto_tiles.len(), 31);
        assert_eq!(save.part1.tail_vars.len(), 11);
        assert_eq!(save.part1.vars2_new.len(), 192);
        assert!(save.part1.objects.is_empty());

        assert_eq!(save.part2.vars3_21, [0; 19]);
        assert!(save.part2.block100.is_some());
        assert!(save.part2.bgm.is_some());
        assert!(save.part2.block109.is_some());
        assert!(save.part2.mds7_9.is_some());
        assert!(save.part2.block137.is_some());
        assert!(save.part2.block8a.is_some());

        // Part3 row 0 is the 800 normal variables; events 0..=549 each have 100 self
        // numbers and 5 self strings.
        assert_eq!(save.part3.row_counts, vec![800]);
        assert_eq!(save.part3.normal_var(0), Some(0));
        assert_eq!(save.part3.normal_var(799), Some(0));
        assert_eq!(save.part3.normal_var(800), None);
        assert_eq!(save.part3.self_var(0, 0), Some(0));
        assert_eq!(save.part3.self_var(549, 99), Some(0));
        assert_eq!(save.part3.self_var(550, 0), None);
        assert_eq!(save.part3.self_string(549, 4), Some(""));
        assert_eq!(save.part3.self_string(550, 0), None);

        assert!(save.part4.objects.is_empty());
        assert_eq!(save.part4.lead.var1, 0xFFFF_FFFE);
        assert_eq!(save.part5.var1, 0);
        assert!(save.part5.events.is_empty());
        assert_eq!(save.vardb.unknown, 0x6F);
        assert_eq!(save.vardb.type_count, 0);
        assert!(save.vardb.types.is_empty());
        assert_eq!(save.part7.var1, 0);
        assert!(save.part7.entries.is_empty());
        assert_eq!(save.chunk(PART7_ID).expect("part 7").len, 1);
    }
}
