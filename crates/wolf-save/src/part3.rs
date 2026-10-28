//! `SavePart3` — the map and event runtime tables.
//!
//! This is the largest segment (218,610 bytes in `SaveData01.sav`). Layout
//! source: `docs/vendor/WolfSave/SavePart3.h`, `Parse` at line 38. Note that
//! every nesting level sits behind a signed test, so a negative count silently
//! truncates the whole tail of the segment — that is the reference behaviour
//! and it is reproduced here. See `docs/SAVE_SPEC.md` §5.

use wolf_format::bytes::Reader;
use wolf_format::error::{Error, Result};

use crate::{ensure_fits, mem_data};

/// One entry of the `m_vars1`/`m_vars2`/`m_vars3` table: a count and a list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavePart3Group {
    /// `m_vars1` — the group's element count.
    pub count: u32,
    /// `m_vars2` — one byte per element.
    pub bytes: Vec<u8>,
    /// `m_vars3` — the words collected across all elements of this group.
    pub words: Vec<u32>,
}

/// One entry of the `m_vars4`/`m_vars5` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavePart3Row {
    /// `m_vars4` — the row's element count.
    pub count: u32,
    /// `m_vars5` — the row's words.
    pub words: Vec<u32>,
}

/// One entry of the `m_vars6`/`m_vars7` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavePart3List {
    /// `m_vars6` — one byte per entry.
    pub count: u8,
    /// `m_vars7` — the words collected across all entries.
    pub words: Vec<u32>,
}

/// One entry of the `m_vars8`/`m_mds2` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavePart3Slot {
    /// `m_vars8` — the number of strings that follow; 0 reads nothing.
    pub count: u8,
    /// `m_mds2New` (`m_mds2Old` below `0x6F`).
    pub strings: Vec<Vec<u8>>,
}

/// The map and event runtime tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePart3 {
    /// `m_var1` — differs per save; looks like a timestamp or seed.
    pub var1: u32,
    /// `m_var2` — 2 here; the number of groups.
    pub var2: u32,
    /// `m_vars1` — the group count column.
    pub group_counts: Vec<u32>,
    /// `m_vars2` — the per-element byte column.
    pub group_bytes: Vec<u8>,
    /// `m_vars3` — the words collected across the groups.
    pub group_words: Vec<u32>,
    /// `m_var3` — 10 here.
    pub var3: u32,
    /// `m_vars4` — the row count column.
    pub row_counts: Vec<u32>,
    /// `m_vars5` — the words collected across the rows.
    pub row_words: Vec<u32>,
    /// `m_var4` — 26 here; the number of `MemData` entries.
    pub var4: u32,
    /// `m_mds1New` — 26 strings here, starting with `"周一"` and a save-thumbnail path.
    pub mds1: Vec<Vec<u8>>,
    /// `m_var5` — 500 here (550 in two saves); the number of lists.
    pub var5: u32,
    /// `m_vars6` — one byte per list.
    pub list_counts: Vec<u8>,
    /// `m_vars7` — the words collected across the lists.
    pub list_words: Vec<u32>,
    /// `m_var6` — 500 here (550 in two saves); the number of slots.
    pub var6: u32,
    /// `m_vars8` — one byte per slot.
    pub slot_counts: Vec<u8>,
    /// `m_mds2New` — the strings collected across the slots.
    pub slot_strings: Vec<Vec<u8>>,
}

pub(crate) fn parse(r: &mut Reader<'_>, file_version: u16) -> Result<SavePart3> {
    let var1 = r.u32()?;
    let var2 = r.u32()?;

    let mut part = SavePart3 {
        var1,
        var2,
        group_counts: Vec::new(),
        group_bytes: Vec::new(),
        group_words: Vec::new(),
        var3: 0,
        row_counts: Vec::new(),
        row_words: Vec::new(),
        var4: 0,
        mds1: Vec::new(),
        var5: 0,
        list_counts: Vec::new(),
        list_words: Vec::new(),
        var6: 0,
        slot_counts: Vec::new(),
        slot_strings: Vec::new(),
    };

    if (var2 as i32) < 0 {
        return Ok(part);
    }

    for _ in 0..var2 {
        let count_pos = r.pos();
        let count = r.u32()?;
        if (count as i32) < 0 {
            return Err(Error::field(
                "SavePart3 group count",
                count_pos,
                format!(
                    "{count:#010X} is negative; the reference implementation aborts the segment"
                ),
            ));
        }
        part.group_counts.push(count);
        ensure_fits(r, count as usize, 1, "SavePart3 group bytes")?;
        for _ in 0..count {
            let per = r.u8()?;
            part.group_bytes.push(per);
            ensure_fits(r, usize::from(per), 4, "SavePart3 group words")?;
            for _ in 0..per {
                part.group_words.push(r.u32()?);
            }
        }
    }

    part.var3 = r.u32()?;
    if part.var3 > 0x270F {
        return Ok(part);
    }

    if (part.var3 as i32) > 0 {
        for _ in 0..part.var3 {
            let count_pos = r.pos();
            let count = r.u32()?;
            if (count as i32) < 0 {
                return Err(Error::field(
                    "SavePart3 row count",
                    count_pos,
                    format!("{count:#010X} is negative; the reference implementation aborts the segment"),
                ));
            }
            part.row_counts.push(count);
            ensure_fits(r, count as usize, 4, "SavePart3 row words")?;
            for _ in 0..count {
                part.row_words.push(r.u32()?);
            }
        }
    }

    part.var4 = r.u32()?;
    if (part.var4 as i32) < 0 {
        return Ok(part);
    }

    let size: u8 = if file_version < 0x6F { 2 } else { 4 };
    ensure_fits(r, part.var4 as usize, usize::from(size), "SavePart3 mds1")?;
    for _ in 0..part.var4 {
        part.mds1.push(mem_data(r, size)?);
    }

    part.var5 = r.u32()?;
    let count_pos = r.pos();
    if (part.var5 as i32) < 0 || part.var5 > 10_000 {
        return Err(Error::field(
            "SavePart3 var5",
            count_pos,
            format!("{} is outside 0..=10000", part.var5),
        ));
    }

    for _ in 0..part.var5 {
        let per = r.u8()?;
        part.list_counts.push(per);
        ensure_fits(r, usize::from(per), 4, "SavePart3 list words")?;
        for _ in 0..per {
            part.list_words.push(r.u32()?);
        }
    }

    part.var6 = r.u32()?;
    if part.var6 > 10_000 {
        return Ok(part);
    }
    let count_pos = r.pos();
    if (part.var6 as i32) <= 0 {
        return Err(Error::field(
            "SavePart3 var6",
            count_pos,
            format!(
                "{:#010X} is zero or negative while the segment still has data; the reference implementation rejects it",
                part.var6
            ),
        ));
    }

    for _ in 0..part.var6 {
        let per = r.u8()?;
        part.slot_counts.push(per);
        if per != 0 {
            ensure_fits(
                r,
                usize::from(per),
                usize::from(size),
                "SavePart3 slot strings",
            )?;
            for _ in 0..per {
                part.slot_strings.push(mem_data(r, size)?);
            }
        }
    }

    Ok(part)
}
