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
