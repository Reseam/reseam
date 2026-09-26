// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;
use std::ops::Range;

use reseam_dex::file::DexBytes;

use super::entry::{self, ResEntry};
use super::{MAX_TYPE_ENTRIES, RES_TABLE_TYPE_TYPE};
use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::chunk::write_header;
use crate::error::{invalid, malformed, Result};
use crate::value::ResValue;

const HEADER_LEN: usize = 20;
const NO_ENTRY: u32 = 0xFFFF_FFFF;
const FLAG_SPARSE: u8 = 0x01;
const FLAG_OFFSET16: u8 = 0x02;

#[derive(Debug, Clone)]
enum EntryOffsets {
    Dense32,
    Dense16,
    Sparse(BTreeMap<u16, u32>),
}

/// One configuration of a resource type.
///
/// Dense 32-bit, dense 16-bit, and sparse entry indexes are supported. Entry
/// payloads are decoded only when accessed. A malformed payload can be read
/// through [`Self::entry_checked`] as an error without hiding other entries.
/// Unchanged chunks are copied verbatim; edited chunks retain their original
/// payload block and append replacement entries behind a new dense index.
#[derive(Debug, Clone)]
pub struct ResType {
    pub id: u8,
    data: DexBytes,
    chunk: Range<usize>,
    header_size: usize,
    config: Vec<u8>,
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
            data: DexBytes::default(),
            chunk: 0..0,
            header_size: 0,
            config,
            raw_len: 0,
            entries_start: 0,
            offsets: EntryOffsets::Dense32,
            overlay: BTreeMap::new(),
            len: 0,
        }
    }

    pub(super) fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "res type")?;
        if header_size < HEADER_LEN {
            return Err(malformed("res type", chunk.start, "header is too short"));
        }
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
        let flags = buf[9];
        if flags & !(FLAG_SPARSE | FLAG_OFFSET16) != 0 {
            return Err(malformed(
                "res type",
                chunk.start + 9,
                "unknown entry index encoding",
            ));
        }
        let index_width = if flags & FLAG_SPARSE == 0 && flags & FLAG_OFFSET16 != 0 {
            2
        } else {
            4
        };
        require_len(
            buf,
            header_size,
            entry_count * index_width,
            "res type offsets",
        )?;
        if entries_start < header_size + entry_count * index_width {
            return Err(malformed(
                "res type",
                chunk.start + 16,
                "entry data overlaps the offset index",
            ));
        }
        let config_end = header_size.min(buf.len());
        if config_end > HEADER_LEN && config_end - HEADER_LEN < 4 {
            return Err(malformed(
                "res type",
                HEADER_LEN,
                "config data is shorter than the size field",
            ));
        }
        let offsets = if flags & FLAG_SPARSE != 0 {
            EntryOffsets::Sparse(
                (0..entry_count)
                    .map(|i| {
                        let at = header_size + i * 4;
                        Ok((
                            read_u16_le(buf, at, "res sparse index")?,
                            read_u16_le(buf, at + 2, "res sparse offset")? as u32 * 4,
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?,
            )
        } else if flags & FLAG_OFFSET16 != 0 {
            EntryOffsets::Dense16
        } else {
            EntryOffsets::Dense32
        };
        let raw_len = match &offsets {
            EntryOffsets::Sparse(entries) => entries
                .last_key_value()
                .map_or(0, |(&index, _)| index as usize + 1),
            EntryOffsets::Dense32 | EntryOffsets::Dense16 => entry_count,
        };
        // Individual entries are decoded on access. Their errors do not
        // prevent unrelated entries in the table from being read.
        Ok(Self {
            id: buf[8],
            data: data.clone(),
            chunk: chunk.clone(),
            header_size,
            config: buf[HEADER_LEN..config_end].to_vec(),
            raw_len,
            entries_start: chunk.start + entries_start,
            offsets,
            overlay: BTreeMap::new(),
            len: raw_len,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The size of the chunk's `ResTable_config` block, so a chunk this crate
    /// creates matches the shape the app's own already have.
    pub(crate) fn config_len(&self) -> usize {
        self.config.len()
    }

    /// The raw `ResTable_config` block, size field included.
    pub fn config(&self) -> &[u8] {
        &self.config
    }

    /// Whether the chunk is the configuration the loader falls back to.
    pub fn is_default_config(&self) -> bool {
        self.config.len() <= 4 || self.config[4..].iter().all(|&b| b == 0)
    }

    /// Screen density from ResTable_config; zero denotes the default density.
    pub(crate) fn density(&self) -> u16 {
        self.config
            .get(14..16)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .unwrap_or(0)
    }

    /// The entry at `i`, decoded; `None` for an absent or malformed entry.
    pub fn entry(&self, i: usize) -> Option<ResEntry> {
        self.entry_checked(i).ok().flatten()
    }

    /// Decode the entry at `i`.
    ///
    /// Returns `Ok(None)` for an index outside this type or one marked absent.
    /// Returns an error for a present entry with invalid structure or bounds;
    /// other entries remain readable.
    pub fn entry_checked(&self, i: usize) -> Result<Option<ResEntry>> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return Ok(entry.clone());
        }
        self.raw_entry_bytes(i)?.map(entry::parse_entry).transpose()
    }

    /// The entry's key and, for a simple entry, its value, without decoding
    /// a complex entry's map.
    pub(super) fn entry_head(&self, i: usize) -> Option<(u32, Option<ResValue>)> {
        self.entry_head_checked(i).ok().flatten()
    }

    /// Read an entry's key without interpreting its value or declared size.
    /// This lets a name lookup identify its target despite malformed contents.
    pub(super) fn entry_key_checked(&self, i: usize) -> Result<Option<u32>> {
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
            read_u16_le(chunk, pos, "compact res entry")? as u32
        } else {
            read_u32_le(chunk, pos + 4, "res entry")?
        };
        Ok(Some(key))
    }

    /// Read the key and simple value, reporting malformed target entries.
    pub(super) fn entry_head_checked(&self, i: usize) -> Result<Option<(u32, Option<ResValue>)>> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return Ok(entry.as_ref().map(|entry| {
                let value = match entry.value {
                    super::EntryValue::Simple(value) => Some(value),
                    super::EntryValue::Complex { .. } => None,
                };
                (entry.key, value)
            }));
        }
        self.raw_entry_bytes(i)?
            .map(|bytes| {
                entry::entry_head(bytes)
                    .ok_or_else(|| invalid("res entry", "could not read entry header"))
            })
            .transpose()
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

    /// A raw entry's indexed offset, without interpreting its contents.
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
                (offset != u16::MAX).then_some(offset as u32 * 4)
            }
            EntryOffsets::Sparse(entries) => entries.get(&(i as u16)).copied(),
        })
    }

    /// The bytes of a raw entry, `None` when the file marks it absent.
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

    /// Plan a verbatim copy or a dense index over the original data and edits.
    /// Untouched entries remain opaque, including entries this reader cannot decode.
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
        let raw_data = &self.data.as_bytes()[self.entries_start..self.chunk.end];
        let mut overlay_bytes = Vec::new();
        let mut offsets = Vec::with_capacity(self.len);
        for i in 0..self.len {
            let offset = match self.overlay.get(&(i as u32)) {
                Some(Some(entry)) => {
                    let padding = (4 - (raw_data.len() + overlay_bytes.len()) % 4) % 4;
                    overlay_bytes.resize(overlay_bytes.len() + padding, 0);
                    let offset = u32::try_from(raw_data.len() + overlay_bytes.len())
                        .map_err(|_| invalid("res type", "entry data exceeds 4 GiB"))?;
                    entry::serialize_entry(&mut overlay_bytes, entry);
                    Some(offset)
                }
                Some(None) => None,
                None => self.raw_offset(i)?,
            };
            offsets.push(offset);
        }
        let entries_start = HEADER_LEN + config.len() + self.len * 4;
        let size = entries_start + raw_data.len() + overlay_bytes.len();
        u32::try_from(size).map_err(|_| invalid("res type", "chunk exceeds 4 GiB"))?;
        Ok(TypePlan {
            res_type: self,
            size,
            rebuilt: Some(RebuiltType {
                config,
                offsets,
                raw_data,
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
    offsets: Vec<Option<u32>>,
    raw_data: &'a [u8],
    overlay_bytes: Vec<u8>,
    entries_start: usize,
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
        for offset in &plan.offsets {
            write_u32(&mut head, offset.unwrap_or(NO_ENTRY));
        }
        out.write_all(&head)?;
        out.write_all(plan.raw_data)?;
        out.write_all(&plan.overlay_bytes)?;
        Ok(())
    }
}
