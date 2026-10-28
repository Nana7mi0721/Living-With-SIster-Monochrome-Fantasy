//! VariableDatabase — every variable type's field config and every row.
//!
//! Layout source: docs/vendor/WolfSave/VariableDatabase.h, Parse at line 281
//! and the nested VariableType::Parse / TypeData::Parse at lines 162 and 56.
//!
//! Two details there are easy to get wrong and both matter:
//!
//! * A row's numbers are read first, all of them, and only then its strings —
//!   not interleaved in field order. The reference implementation then sorts the
//!   fields by id, so the in-memory order differs from the on-disk order.
//! * The field list (m_typeConfig) is stored per type and tells you which ids are
//!   numeric (below 2000) and which are textual (2000 and above). The field
//!   names are not in the save at all; they live in
//!   Data/BasicData/CDataBase.project. This crate deliberately does not read
//!   that file — see docs/SAVE_SPEC.md section 9.
#![allow(clippy::module_name_repetitions)]

use wolf_format::bytes::Reader;
use wolf_format::error::Result;

use crate::{ensure_fits, mem_data};

/// One field of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableField {
    /// The field id — the index into the type's config list.
    pub id: u32,
    /// The value.
    pub value: VariableValue,
}

/// A field value: either a number or a MemData<DWORD> string body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableValue {
    /// An int32 from a field whose config entry is below 2000.
    Number(i32),
    /// A string body from a field whose config entry is 2000 or above.
    String(Vec<u8>),
}

/// One row of a type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VariableRow {
    /// The row index, m_idx in the source.
    pub index: u32,
    /// Every field, sorted by id exactly as the C++ sorts them.
    pub fields: Vec<VariableField>,
}

impl VariableRow {
    /// The numeric value of a field, if it is one.
    #[must_use]
    pub fn number(&self, id: u32) -> Option<i32> {
        self.fields
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| match f.value {
                VariableValue::Number(n) => Some(n),
                VariableValue::String(_) => None,
            })
    }

    /// The string body of a field, if it is one.
    #[must_use]
    pub fn string(&self, id: u32) -> Option<&[u8]> {
        self.fields
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| match &f.value {
                VariableValue::String(s) => Some(s.as_slice()),
                VariableValue::Number(_) => None,
            })
    }
}

/// One variable type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableType {
    /// m_unknown — the first word, which doubles as the field count when positive.
    pub unknown: i32,
    /// m_dis — only read when m_unknown is -2 or lower. 0 means this type has no
    /// separate field names, which is why it also decides how rows are labelled.
    pub dis: i32,
    /// m_fieldCount — only set when m_unknown is -1 or lower.
    pub field_count: u32,
    /// m_typeConfig — the per-field config words. Below 2000 means the field is
    /// numeric, 2000 and above means it is textual.
    pub config: Vec<u32>,
    /// m_typeDataCount — the number of rows.
    pub row_count: u32,
    /// The rows.
    pub rows: Vec<VariableRow>,
}

impl VariableType {
    /// How many of the fields are numeric.
    #[must_use]
    pub fn number_field_count(&self) -> usize {
        self.config.iter().filter(|def| **def < 2000).count()
    }

    /// How many of the fields are textual.
    #[must_use]
    pub fn string_field_count(&self) -> usize {
        self.config.iter().filter(|def| **def >= 2000).count()
    }
}

/// The variable database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDatabase {
    /// m_unknown — one byte the reference implementation reads and skips. The
    /// source comments on it: "Is here 1 byte just skipped?" It is 0x6F here.
    pub unknown: u8,
    /// m_typeCount — 30 here.
    pub type_count: u32,
    /// The types.
    pub types: Vec<VariableType>,
}

pub(crate) fn parse(r: &mut Reader<'_>) -> Result<VariableDatabase> {
    let unknown = r.u8()?;
    let type_count = r.u32()?;
    // A type costs at least 8 bytes: its first word and the row count.
    ensure_fits(r, type_count as usize, 8, "VariableDatabase types")?;
    let mut types = Vec::with_capacity(type_count as usize);
    for _ in 0..type_count {
        types.push(VariableType::parse(r)?);
    }
    Ok(VariableDatabase {
        unknown,
        type_count,
        types,
    })
}

impl VariableType {
    pub(crate) fn parse(r: &mut Reader<'_>) -> Result<VariableType> {
        let unknown = r.i32()?;
        let mut dis = 0;
        let mut field_count = 0;
        let mut count = unknown;
        if unknown <= -1 {
            if unknown <= -2 {
                dis = r.i32()?;
            }
            let pos = r.pos();
            field_count = r.u32()?;
            if field_count > i32::MAX as u32 {
                return Err(wolf_format::error::Error::field(
                    "VariableType field count",
                    pos,
                    format!("{field_count} fields does not fit in an int32"),
                ));
            }
            count = field_count as i32;
        }

        let mut config = Vec::new();
        if count > 0 {
            ensure_fits(r, count as usize, 4, "VariableType config")?;
            config = Vec::with_capacity(count as usize);
            for _ in 0..count {
                config.push(r.u32()?);
            }
        }

        let row_count = r.u32()?;
        // Every row costs at least one number per numeric field.
        let numeric = config.iter().filter(|def| **def < 2000).count();
        ensure_fits(r, row_count as usize, numeric * 4, "VariableType rows")?;
        let mut rows = Vec::with_capacity(row_count as usize);
        for index in 0..row_count {
            rows.push(VariableRow::parse(r, &config, index)?);
        }

        Ok(VariableType {
            unknown,
            dis,
            field_count,
            config,
            row_count,
            rows,
        })
    }
}

impl VariableRow {
    fn parse(r: &mut Reader<'_>, config: &[u32], index: u32) -> Result<VariableRow> {
        let mut fields = Vec::with_capacity(config.len());

        // First pass: every numeric field, in config order.
        for (id, def) in config.iter().enumerate() {
            if *def < 2000 {
                fields.push(VariableField {
                    id: id as u32,
                    value: VariableValue::Number(r.i32()?),
                });
            }
        }

        // Second pass: every textual field, in config order.
        for (id, def) in config.iter().enumerate() {
            if *def >= 2000 {
                fields.push(VariableField {
                    id: id as u32,
                    value: VariableValue::String(mem_data(r, 4)?),
                });
            }
        }

        // The C++ sorts by id, which merges the two passes back into field order.
        fields.sort_by_key(|field| field.id);

        Ok(VariableRow { index, fields })
    }
}

/// Convenience for callers that only want to know how a type is shaped.
impl std::fmt::Display for VariableType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "type(unknown={}, dis={}, fields={}, rows={}, {}+{})",
            self.unknown,
            self.dis,
            self.config.len(),
            self.row_count,
            self.number_field_count(),
            self.string_field_count()
        )
    }
}
