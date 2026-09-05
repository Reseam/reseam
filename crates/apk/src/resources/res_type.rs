// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;
use std::ops::Range;

use reseam_dex::file::DexBytes;

use super::entry::{self, ResEntry};
use super::{MAX_TYPE_ENTRIES, RES_TABLE_TYPE_TYPE};
use crate::buf::{read_u32_le, require_len, write_u16, write_u32};
use crate::chunk::write_header;
use crate::error::{invalid, malformed, Result};
use crate::value::ResValue;

const HEADER_LEN: usize = 20;
const NO_ENTRY: u32 = 0xFFFF_FFFF;

/// A `ResTable_type` chunk (one configuration of one type): entries are read
/// from the chunk, with added or changed entries in an overlay.
#[derive(Debug, Clone)]
pub struct ResType {
    pub id: u8,
    data: DexBytes,
    chunk: Range<usize>,
    header_size: usize,
    config: Vec<u8>,
    raw_len: usize,
    entries_start: usize,
    overlay: BTreeMap<u32, Option<ResEntry>>,
    len: usize,
}

impl ResType {
    pub fn new(id: u8, config: Vec<u8>) -> Self {
        Self {
            id,
            data: DexBytes::default(),
            chunk: 0..0,
            header_size: 0,
            config,
            raw_len: 0,
            entries_start: 0,
            overlay: BTreeMap::new(),
            len: 0,
        }
    }

    pub(super) fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "res type")?;
        let entry_count = read_u32_le(buf, 12, "res type")? as usize;
        if entry_count > MAX_TYPE_ENTRIES {
            return Err(invalid("res type", "entry count exceeds safety limit"));
        }
        let entries_start = read_u32_le(buf, 16, "res type")? as usize;
        if entries_start > buf.len() {
            return Err(malformed(
                "res type",
                16,
                "entries start is outside type chunk",
            ));
        }
        require_len(buf, header_size, entry_count * 4, "res type offsets")?;
        let config_end = header_size.min(buf.len());
        if config_end > HEADER_LEN && config_end - HEADER_LEN < 4 {
            return Err(malformed(
                "res type",
                HEADER_LEN,
                "config data is shorter than the size field",
            ));
        }
        let res_type = Self {
            id: buf[8],
            data: data.clone(),
            chunk: chunk.clone(),
            header_size,
            config: buf[HEADER_LEN..config_end].to_vec(),
            raw_len: entry_count,
            entries_start: chunk.start + entries_start,
            overlay: BTreeMap::new(),
            len: entry_count,
        };
        for i in 0..entry_count {
            res_type.raw_entry_bytes(i)?;
        }
        Ok(res_type)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn is_default_config(&self) -> bool {
        self.config.len() <= 4 || self.config[4..].iter().all(|&b| b == 0)
    }

    /// The entry at `i`, decoded; `None` for an absent entry.
    pub fn entry(&self, i: usize) -> Option<ResEntry> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return entry.clone();
        }
        entry::parse_entry(self.raw_entry_bytes(i).ok()??).ok()
    }

    /// The entry's key and, for a simple entry, its value, without decoding
    /// a complex entry's map.
    pub(super) fn entry_head(&self, i: usize) -> Option<(u32, Option<ResValue>)> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            let entry = entry.as_ref()?;
            let value = match entry.value {
                super::EntryValue::Simple(value) => Some(value),
                super::EntryValue::Complex { .. } => None,
            };
            return Some((entry.key, value));
        }
        entry::entry_head(self.raw_entry_bytes(i).ok()??)
    }

    pub fn set(&mut self, i: usize, entry: Option<ResEntry>) {
        self.len = self.len.max(i + 1);
        self.overlay.insert(i as u32, entry);
    }

    pub fn push(&mut self, entry: Option<ResEntry>) -> usize {
        let i = self.len;
        self.set(i, entry);
        i
    }

    /// Grows the entry list with absent entries up to `len`.
    pub(crate) fn pad_to(&mut self, len: usize) {
        self.len = self.len.max(len);
    }

    /// The bytes of a raw entry, `None` when the file marks it absent.
    fn raw_entry_bytes(&self, i: usize) -> Result<Option<&[u8]>> {
        if i >= self.raw_len {
            return Ok(None);
        }
        let data = self.data.as_bytes();
        let offset = read_u32_le(
            data,
            self.chunk.start + self.header_size + i * 4,
            "res type offset",
        )?;
        if offset == NO_ENTRY {
            return Ok(None);
        }
        let pos = self.entries_start + offset as usize;
        let chunk = &data[..self.chunk.end];
        let len = entry::entry_len(chunk, pos)?;
        Ok(Some(&chunk[pos..pos + len]))
    }

    /// Lays the chunk out for writing: verbatim when untouched, otherwise
    /// raw entry bytes copied through with overlay entries encoded once.
    pub(super) fn plan(&self) -> Result<TypePlan<'_>> {
        if self.overlay.is_empty() && self.len == self.raw_len && !self.chunk.is_empty() {
            return Ok(TypePlan {
                res_type: self,
                size: self.chunk.len(),
                rebuilt: None,
            });
        }
        let config = if self.config.is_empty() {
            Cow::Owned(4u32.to_le_bytes().to_vec())
        } else {
            Cow::Borrowed(self.config.as_slice())
        };
        let mut overlay_bytes = Vec::new();
        let mut entries = Vec::with_capacity(self.len);
        let mut data_len = 0usize;
        for i in 0..self.len {
            let source = match self.overlay.get(&(i as u32)) {
                Some(Some(entry)) => {
                    let start = overlay_bytes.len();
                    entry::serialize_entry(&mut overlay_bytes, entry);
                    EntrySource::Overlay(start..overlay_bytes.len())
                }
                Some(None) => EntrySource::Absent,
                None => match self.raw_entry_bytes(i)? {
                    Some(bytes) => EntrySource::Raw(bytes.len()),
                    None => EntrySource::Absent,
                },
            };
            data_len += source.len();
            entries.push(source);
        }
        let entries_start = HEADER_LEN + config.len() + self.len * 4;
        Ok(TypePlan {
            res_type: self,
            size: entries_start + data_len,
            rebuilt: Some(RebuiltType {
                config,
                entries,
                overlay_bytes,
                entries_start,
            }),
        })
    }
}

pub(super) struct TypePlan<'a> {
    res_type: &'a ResType,
    pub size: usize,
    rebuilt: Option<RebuiltType<'a>>,
}

struct RebuiltType<'a> {
    config: Cow<'a, [u8]>,
    entries: Vec<EntrySource>,
    overlay_bytes: Vec<u8>,
    entries_start: usize,
}

enum EntrySource {
    Raw(usize),
    Overlay(Range<usize>),
    Absent,
}

impl EntrySource {
    fn len(&self) -> usize {
        match self {
            Self::Raw(len) => *len,
            Self::Overlay(range) => range.len(),
            Self::Absent => 0,
        }
    }
}

impl TypePlan<'_> {
    pub(super) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let res_type = self.res_type;
        let Some(plan) = &self.rebuilt else {
            out.write_all(&res_type.data.as_bytes()[res_type.chunk.clone()])?;
            return Ok(());
        };
        let mut head = Vec::with_capacity(plan.entries_start);
        write_header(
            &mut head,
            RES_TABLE_TYPE_TYPE,
            (HEADER_LEN + plan.config.len()) as u16,
            self.size,
        );
        head.push(res_type.id);
        head.push(0);
        write_u16(&mut head, 0);
        write_u32(&mut head, res_type.len as u32);
        write_u32(&mut head, plan.entries_start as u32);
        head.extend_from_slice(&plan.config);
        let mut offset = 0u32;
        for source in &plan.entries {
            match source {
                EntrySource::Absent => write_u32(&mut head, NO_ENTRY),
                present => {
                    write_u32(&mut head, offset);
                    offset += present.len() as u32;
                }
            }
        }
        out.write_all(&head)?;
        for (i, source) in plan.entries.iter().enumerate() {
            match source {
                EntrySource::Raw(_) => out.write_all(res_type.raw_entry_bytes(i)?.unwrap())?,
                EntrySource::Overlay(range) => out.write_all(&plan.overlay_bytes[range.clone()])?,
                EntrySource::Absent => {}
            }
        }
        Ok(())
    }
}
