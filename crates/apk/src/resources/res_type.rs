// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod reader;
mod writer;
pub(super) use writer::TypePlan;

use std::collections::BTreeMap;
use std::ops::Range;

use reseam_storage::Bytes;

use super::entry::{self, ResEntry};
use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::error::{Result, invalid};
use crate::value::ResValue;

const HEADER_LEN: usize = 20;
const NO_ENTRY: u32 = 0xFFFF_FFFF;
const FLAG_SPARSE: u8 = 0x01;
const FLAG_OFFSET16: u8 = 0x02;

#[derive(Debug, Clone)]
enum EntryOffsets {
    Dense32,
    Dense16,
    Sparse { count: usize },
}

/// One configuration of a resource type.
///
/// Dense 32-bit, dense 16-bit, and sparse entry indexes are supported. Entry
/// payloads are decoded only when accessed. A malformed payload can be read
/// through [`Self::entry`] as an error without hiding other entries.
/// Unchanged chunks are copied verbatim; edited chunks retain their original
/// payload block and append replacement entries behind a new dense index.
#[derive(Debug, Clone)]
pub struct ResType {
    pub(super) id: u8,
    data: Bytes,
    chunk: Range<usize>,
    header_size: usize,
    config: Option<Vec<u8>>,
    raw_len: usize,
    entries_start: usize,
    offsets: EntryOffsets,
    overlay: BTreeMap<u32, Option<ResEntry>>,
    len: usize,
}

impl ResType {
    pub fn new(id: u8, config: Vec<u8>) -> Self {
        Self {
            id,
            data: Bytes::default(),
            chunk: 0..0,
            header_size: 0,
            config: Some(config),
            raw_len: 0,
            entries_start: 0,
            offsets: EntryOffsets::Dense32,
            overlay: BTreeMap::new(),
            len: 0,
        }
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn config_len(&self) -> usize {
        self.config().len()
    }

    /// The raw `ResTable_config` block, size field included.
    pub fn config(&self) -> &[u8] {
        self.config.as_deref().unwrap_or_else(|| {
            &self.data.as_bytes()
                [self.chunk.start + HEADER_LEN..self.chunk.start + self.header_size]
        })
    }

    /// Whether the chunk is the configuration the loader falls back to.
    pub fn is_default_config(&self) -> bool {
        let config = self.config();
        config.len() <= 4 || config[4..].iter().all(|&b| b == 0)
    }

    pub(crate) fn density(&self) -> u16 {
        self.config()
            .get(14..16)
            .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Decode the entry at `i`.
    ///
    /// Returns `Ok(None)` for an index outside this type or one marked absent.
    /// Returns an error for a present entry with invalid structure or bounds;
    /// other entries remain readable.
    pub fn entry(&self, i: usize) -> Result<Option<ResEntry>> {
        if i >= self.len {
            return Ok(None);
        }
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return Ok(entry.clone());
        }
        self.raw_entry_bytes(i)?.map(entry::parse_entry).transpose()
    }

    pub(super) fn entry_key(&self, i: usize) -> Result<Option<u32>> {
        if i >= self.len {
            return Ok(None);
        }
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return Ok(entry.as_ref().map(|entry| entry.key));
        }
        let Some(offset) = self.raw_offset(i)? else {
            return Ok(None);
        };
        let pos = self
            .entries_start
            .checked_add(offset as usize)
            .ok_or_else(|| invalid("res entry", "entry offset overflows address space"))?;
        let chunk = &self.data.as_bytes()[..self.chunk.end];
        require_len(chunk, pos, 8, "res entry")?;
        let flags = read_u16_le(chunk, pos + 2, "res entry")?;
        let key = if flags & entry::FLAG_COMPACT != 0 {
            u32::from(read_u16_le(chunk, pos, "compact res entry")?)
        } else {
            read_u32_le(chunk, pos + 4, "res entry")?
        };
        Ok(Some(key))
    }

    pub(super) fn entry_head(&self, i: usize) -> Result<Option<(u32, Option<ResValue>)>> {
        if i >= self.len {
            return Ok(None);
        }
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return Ok(entry.as_ref().map(|entry| {
                let value = match entry.value {
                    super::EntryValue::Simple(value) => Some(value),
                    super::EntryValue::Complex { .. } => None,
                };
                (entry.key, value)
            }));
        }
        self.raw_entry_bytes(i)?.map(entry::entry_head).transpose()
    }

    /// Replaces an entry or marks it absent, extending this configuration when needed.
    /// Indices outside the 16-bit resource-ID range are errors and leave it unchanged.
    pub fn set(&mut self, i: usize, entry: Option<ResEntry>) -> Result<()> {
        let index =
            u16::try_from(i).map_err(|_| invalid("resource entry", "entry index exceeds 65535"))?;
        self.len = self.len.max(i + 1);
        self.overlay.insert(u32::from(index), entry);
        Ok(())
    }

    /// Appends an entry, returning its index. A full 16-bit index space is an error.
    pub fn push(&mut self, entry: Option<ResEntry>) -> Result<usize> {
        let i = self.len;
        self.set(i, entry)?;
        Ok(i)
    }

    pub(crate) fn pad_to(&mut self, len: usize) {
        self.len = self.len.max(len);
    }

    fn raw_offset(&self, i: usize) -> Result<Option<u32>> {
        if i >= self.raw_len {
            return Ok(None);
        }
        let data = self.data.as_bytes();
        Ok(match &self.offsets {
            EntryOffsets::Dense32 => {
                let offset = read_u32_le(
                    data,
                    self.chunk.start + self.header_size + i * 4,
                    "res type offset",
                )?;
                (offset != NO_ENTRY).then_some(offset)
            }
            EntryOffsets::Dense16 => {
                let offset = read_u16_le(
                    data,
                    self.chunk.start + self.header_size + i * 2,
                    "res type offset",
                )?;
                (offset != u16::MAX).then_some(u32::from(offset) * 4)
            }
            EntryOffsets::Sparse { count } => {
                let index = &data[self.chunk.start + self.header_size..][..count * 4];
                let records = index.as_chunks::<4>().0;
                let at = records.partition_point(|record| {
                    usize::from(u16::from_le_bytes([record[0], record[1]])) < i
                });
                records.get(at).and_then(|record| {
                    (usize::from(u16::from_le_bytes([record[0], record[1]])) == i)
                        .then(|| u32::from(u16::from_le_bytes([record[2], record[3]])) * 4)
                })
            }
        })
    }

    fn raw_entry_bytes(&self, i: usize) -> Result<Option<&[u8]>> {
        let Some(offset) = self.raw_offset(i)? else {
            return Ok(None);
        };
        let pos = self
            .entries_start
            .checked_add(offset as usize)
            .ok_or_else(|| invalid("res entry", "entry offset overflows address space"))?;
        let chunk = &self.data.as_bytes()[..self.chunk.end];
        let len = entry::entry_len(chunk, pos)?;
        Ok(Some(&chunk[pos..pos + len]))
    }
}
