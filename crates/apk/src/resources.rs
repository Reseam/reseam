// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `resources.arsc` as a view over its bytes. String pools, type specs and
//! type chunks are read in place; only entries a patch adds or changes are
//! owned, and serialization copies every untouched chunk verbatim.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::sync::OnceLock;

use reseam_dex::file::DexBytes;
use rustc_hash::FxHasher;

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::{invalid, malformed, Result};
use crate::string_encoding::{encode_utf16, encode_utf8};

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
const MAX_STRING_POOL_STRINGS: usize = 1_000_000;
const MAX_TYPE_ENTRIES: usize = 1_000_000;
const MAX_UTF16_CODE_UNITS: usize = 1_000_000;
const NO_ENTRY: u32 = 0xFFFF_FFFF;
const VALUE_TYPE_STRING: u8 = 0x03;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    pub global_strings: ResStringPool,
    pub packages: Vec<ResPackage>,
}

#[derive(Debug, Clone)]
pub struct ResPackage {
    pub id: u32,
    pub name: String,
    pub type_strings: ResStringPool,
    pub key_strings: ResStringPool,
    pub last_public_type: u32,
    pub last_public_key: u32,
    pub type_id_offset: u32,
    pub type_specs: Vec<TypeSpec>,
    pub types: Vec<ResType>,
}

#[derive(Debug, Clone)]
pub struct ResEntry {
    pub flags: u16,
    pub key: u32,
    pub value: ResValue,
}

#[derive(Debug, Clone)]
pub enum ResValue {
    Simple { data_type: u8, data: u32 },
    Complex { parent: u32, entries: Vec<MapEntry> },
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub name: u32,
    pub data_type: u8,
    pub data: u32,
}

/// `(package id, type id, entry index, simple value)` of a located entry.
type FoundEntry = (u32, u8, usize, Option<(u8, u32)>);

#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub package_id: u32,
    pub type_id: u8,
    pub entry_index: u32,
    pub key_name: String,
}

/// A `ResStringPool_header` chunk read in place, with strings added after
/// parse owned and edits kept as overrides. Style spans stay in the file:
/// they are copied through on rebuild, and appended strings carry none.
#[derive(Debug, Clone, Default)]
pub struct ResStringPool {
    data: DexBytes,
    chunk: Range<usize>,
    raw_len: usize,
    style_count: usize,
    is_utf8: bool,
    offsets_start: usize,
    strings_start: usize,
    styles_start: usize,
    owned: Vec<String>,
    overrides: BTreeMap<u32, String>,
    /// `(hash, index)` of every string, sorted by hash, built on first lookup.
    index: OnceLock<Vec<(u32, u32)>>,
}

impl ResStringPool {
    pub fn new(strings: Vec<String>, is_utf8: bool) -> Self {
        Self {
            is_utf8,
            owned: strings,
            ..Self::default()
        }
    }

    fn parse(data: &DexBytes, chunk: Range<usize>) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, 28, "res string pool")?;
        let header_size = read_u16_le(buf, 2, "res string pool")? as usize;
        let string_count = read_u32_le(buf, 8, "res string pool")? as usize;
        if string_count > MAX_STRING_POOL_STRINGS {
            return Err(invalid("res string pool", "string count exceeds safety limit"));
        }
        let style_count = read_u32_le(buf, 12, "res string pool")? as usize;
        let flags = read_u32_le(buf, 16, "res string pool")?;
        let strings_start = read_u32_le(buf, 20, "res string pool")? as usize;
        let styles_start = read_u32_le(buf, 24, "res string pool")? as usize;
        let is_utf8 = (flags & (1 << 8)) != 0;
        require_len(buf, header_size, (string_count + style_count) * 4, "res string pool offsets")?;
        if strings_start > buf.len() || (style_count > 0 && (styles_start < strings_start || styles_start > buf.len())) {
            return Err(malformed("res string pool", 20, "string or style data start is outside pool"));
        }

        let pool = Self {
            data: data.clone(),
            chunk: chunk.clone(),
            raw_len: string_count,
            style_count,
            is_utf8,
            offsets_start: chunk.start + header_size,
            strings_start: chunk.start + strings_start,
            styles_start: chunk.start + if style_count > 0 { styles_start } else { buf.len() },
            ..Self::default()
        };
        for i in 0..string_count {
            pool.raw(i)?;
        }
        Ok(pool)
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.owned.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_utf8(&self) -> bool {
        self.is_utf8
    }

    pub fn get(&self, index: u32) -> Option<Cow<'_, str>> {
        if let Some(s) = self.overrides.get(&index) {
            return Some(Cow::Borrowed(s));
        }
        let i = index as usize;
        if i < self.raw_len {
            return self.raw(i).ok();
        }
        self.owned.get(i - self.raw_len).map(|s| Cow::Borrowed(s.as_str()))
    }

    pub fn iter(&self) -> impl Iterator<Item = Cow<'_, str>> {
        (0..self.len() as u32).filter_map(move |i| self.get(i))
    }

    pub fn set(&mut self, index: u32, value: String) {
        if (index as usize) < self.len() {
            self.overrides.insert(index, value);
        }
    }

    pub fn push(&mut self, value: &str) -> u32 {
        let index = self.len() as u32;
        self.index.take();
        self.owned.push(value.to_string());
        index
    }

    /// Index of the string equal to `value`, searching the whole pool.
    pub fn find(&self, value: &str) -> Option<u32> {
        if let Some(i) = self.find_added(value) {
            return Some(i);
        }
        let index = self.index.get_or_init(|| {
            let mut entries: Vec<(u32, u32)> = (0..self.raw_len as u32)
                .filter(|i| !self.overrides.contains_key(i))
                .filter_map(|i| self.get(i).map(|s| (hash_str(&s), i)))
                .collect();
            entries.sort_unstable();
            entries
        });
        let hash = hash_str(value);
        let start = index.partition_point(|&(h, _)| h < hash);
        index[start..]
            .iter()
            .take_while(|&&(h, _)| h == hash)
            .map(|&(_, i)| i)
            .find(|&i| self.get(i).as_deref() == Some(value))
    }

    /// Index of the string equal to `value` among strings added or changed
    /// after parse, without indexing the file's own strings.
    fn find_added(&self, value: &str) -> Option<u32> {
        if let Some((&i, _)) = self.overrides.iter().find(|(_, s)| s.as_str() == value) {
            return Some(i);
        }
        self.owned
            .iter()
            .position(|s| s == value)
            .map(|i| (self.raw_len + i) as u32)
    }

    pub fn intern(&mut self, value: &str) -> u32 {
        self.find(value).unwrap_or_else(|| self.push(value))
    }

    /// Like [`Self::intern`] but only deduplicates against strings added
    /// after parse: a value already in the file gets a second entry, which
    /// the format allows, instead of an index over every string in the file.
    pub fn intern_added(&mut self, value: &str) -> u32 {
        self.find_added(value).unwrap_or_else(|| self.push(value))
    }

    /// Lays the pool out for writing: verbatim when untouched, otherwise the
    /// raw string bytes and style tables copied through with appended and
    /// overridden strings encoded after them.
    fn plan(&self) -> PoolPlan<'_> {
        let data = self.data.as_bytes();
        if self.overrides.is_empty() && self.owned.is_empty() && !self.chunk.is_empty() {
            return PoolPlan {
                pool: self,
                size: self.chunk.len(),
                rebuilt: None,
            };
        }
        let count = self.len();
        let raw_region = self.strings_start.min(self.styles_start)..self.styles_start;
        let mut extra = Vec::new();
        let mut offsets = Vec::with_capacity(count);
        for i in 0..count as u32 {
            if (i as usize) < self.raw_len && !self.overrides.contains_key(&i) {
                offsets.push(read_u32_le(data, self.offsets_start + i as usize * 4, "res string offset").unwrap_or(0));
                continue;
            }
            offsets.push((raw_region.len() + extra.len()) as u32);
            let s = self.get(i).unwrap_or_default();
            if self.is_utf8 {
                encode_utf8(&mut extra, &s);
            } else {
                encode_utf16(&mut extra, &s);
            }
        }
        let has_styles = self.style_count > 0;
        if has_styles {
            extra.resize((raw_region.len() + extra.len()).div_ceil(4) * 4 - raw_region.len(), 0);
        }
        let strings_start = 28 + (count + self.style_count) * 4;
        let styles_start = strings_start + raw_region.len() + extra.len();
        let style_data = self.styles_start..self.chunk.end;
        let size = (styles_start + style_data.len() + 3) & !3;
        PoolPlan {
            pool: self,
            size,
            rebuilt: Some(RebuiltPool {
                offsets,
                extra,
                raw_region,
                style_data,
                strings_start,
                styles_start: if has_styles { styles_start } else { 0 },
            }),
        }
    }

    fn raw(&self, i: usize) -> Result<Cow<'_, str>> {
        let data = self.data.as_bytes();
        let offset = read_u32_le(data, self.offsets_start + i * 4, "res string offset")? as usize;
        let abs = self.strings_start + offset;
        if abs >= self.styles_start {
            return Err(malformed("res string pool", abs, "string offset extends past pool"));
        }
        let pool = &data[..self.styles_start];
        if self.is_utf8 {
            decode_res_utf8(pool, abs)
        } else {
            decode_res_utf16(pool, abs).map(Cow::Owned)
        }
    }
}

struct PoolPlan<'a> {
    pool: &'a ResStringPool,
    size: usize,
    rebuilt: Option<RebuiltPool>,
}

struct RebuiltPool {
    offsets: Vec<u32>,
    extra: Vec<u8>,
    raw_region: Range<usize>,
    style_data: Range<usize>,
    strings_start: usize,
    styles_start: usize,
}

impl PoolPlan<'_> {
    fn write(&self, out: &mut dyn Write) -> Result<()> {
        let pool = self.pool;
        let data = pool.data.as_bytes();
        let Some(plan) = &self.rebuilt else {
            out.write_all(&data[pool.chunk.clone()])?;
            return Ok(());
        };
        let mut header = Vec::with_capacity(28);
        write_u16(&mut header, RES_STRING_POOL_TYPE);
        write_u16(&mut header, 28);
        write_u32(&mut header, self.size as u32);
        write_u32(&mut header, plan.offsets.len() as u32);
        write_u32(&mut header, pool.style_count as u32);
        write_u32(&mut header, if pool.is_utf8 { 1 << 8 } else { 0 });
        write_u32(&mut header, plan.strings_start as u32);
        write_u32(&mut header, plan.styles_start as u32);
        out.write_all(&header)?;
        let mut offsets = Vec::with_capacity(plan.offsets.len() * 4);
        for offset in &plan.offsets {
            write_u32(&mut offsets, *offset);
        }
        out.write_all(&offsets)?;
        let style_offsets = pool.offsets_start + pool.raw_len * 4;
        out.write_all(&data[style_offsets..style_offsets + pool.style_count * 4])?;
        out.write_all(&data[plan.raw_region.clone()])?;
        out.write_all(&plan.extra)?;
        out.write_all(&data[plan.style_data.clone()])?;
        let written = 28 + plan.offsets.len() * 4 + pool.style_count * 4 + plan.raw_region.len() + plan.extra.len() + plan.style_data.len();
        out.write_all(&vec![0u8; self.size - written])?;
        Ok(())
    }
}

/// A `ResTable_typeSpec` chunk: the per-entry flags in place plus flags for
/// entries added after parse.
#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub id: u8,
    data: DexBytes,
    flags_start: usize,
    raw_len: usize,
    extra: Vec<u32>,
}

impl TypeSpec {
    pub fn new(id: u8, flags: Vec<u32>) -> Self {
        Self {
            id,
            data: DexBytes::default(),
            flags_start: 0,
            raw_len: 0,
            extra: flags,
        }
    }

    fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(16), "type spec")?;
        let entry_count = read_u32_le(buf, 12, "type spec")? as usize;
        if entry_count > MAX_TYPE_ENTRIES {
            return Err(invalid("type spec", "entry count exceeds safety limit"));
        }
        require_len(buf, header_size, entry_count * 4, "type spec flags")?;
        Ok(Self {
            id: buf[8],
            data: data.clone(),
            flags_start: chunk.start + header_size,
            raw_len: entry_count,
            extra: Vec::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.extra.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn flag(&self, i: usize) -> Option<u32> {
        if i < self.raw_len {
            let data = self.data.as_bytes();
            return read_u32_le(data, self.flags_start + i * 4, "type spec flags").ok();
        }
        self.extra.get(i - self.raw_len).copied()
    }

    pub fn push(&mut self, flag: u32) {
        self.extra.push(flag);
    }

    fn size(&self) -> usize {
        16 + self.len() * 4
    }

    fn write(&self, out: &mut dyn Write) -> Result<()> {
        let mut head = Vec::with_capacity(16 + self.extra.len() * 4);
        write_u16(&mut head, RES_TABLE_TYPE_SPEC);
        write_u16(&mut head, 16);
        write_u32(&mut head, self.size() as u32);
        head.push(self.id);
        head.push(0);
        write_u16(&mut head, 0);
        write_u32(&mut head, self.len() as u32);
        out.write_all(&head)?;
        if self.raw_len > 0 {
            let data = self.data.as_bytes();
            out.write_all(&data[self.flags_start..self.flags_start + self.raw_len * 4])?;
        }
        let mut extra = Vec::with_capacity(self.extra.len() * 4);
        for flag in &self.extra {
            write_u32(&mut extra, *flag);
        }
        out.write_all(&extra)?;
        Ok(())
    }
}

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

    fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(20), "res type")?;
        let entry_count = read_u32_le(buf, 12, "res type")? as usize;
        if entry_count > MAX_TYPE_ENTRIES {
            return Err(invalid("res type", "entry count exceeds safety limit"));
        }
        let entries_start = read_u32_le(buf, 16, "res type")? as usize;
        if entries_start > buf.len() {
            return Err(malformed("res type", 16, "entries start is outside type chunk"));
        }
        require_len(buf, header_size, entry_count * 4, "res type offsets")?;
        let config_end = header_size.min(buf.len());
        if config_end > 20 && config_end - 20 < 4 {
            return Err(malformed("res type", 20, "config data is shorter than the size field"));
        }
        let res_type = Self {
            id: buf[8],
            data: data.clone(),
            chunk: chunk.clone(),
            header_size,
            config: buf[20..config_end].to_vec(),
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

    pub fn is_default_config(&self) -> bool {
        self.config.len() <= 4 || self.config[4..].iter().all(|&b| b == 0)
    }

    /// The entry at `i`, decoded; `None` for an absent entry.
    pub fn entry(&self, i: usize) -> Option<ResEntry> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            return entry.clone();
        }
        let bytes = self.raw_entry_bytes(i).ok()??;
        parse_entry(bytes).ok()
    }

    /// The entry's key and, for a simple value, its type and data, without
    /// decoding a complex value's map.
    fn entry_head(&self, i: usize) -> Option<(u32, Option<(u8, u32)>)> {
        if let Some(entry) = self.overlay.get(&(i as u32)) {
            let entry = entry.as_ref()?;
            let simple = match entry.value {
                ResValue::Simple { data_type, data } => Some((data_type, data)),
                ResValue::Complex { .. } => None,
            };
            return Some((entry.key, simple));
        }
        let bytes = self.raw_entry_bytes(i).ok()??;
        let flags = read_u16_le(bytes, 2, "res entry").ok()?;
        let key = read_u32_le(bytes, 4, "res entry").ok()?;
        let simple = (flags & 0x0001 == 0).then(|| (bytes[11], read_u32_le(bytes, 12, "res value").unwrap_or(0)));
        Some((key, simple))
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
    pub fn pad_to(&mut self, len: usize) {
        self.len = self.len.max(len);
    }

    /// The bytes of a raw entry, `None` when the file marks it absent.
    fn raw_entry_bytes(&self, i: usize) -> Result<Option<&[u8]>> {
        if i >= self.raw_len {
            return Ok(None);
        }
        let data = self.data.as_bytes();
        let offset = read_u32_le(data, self.chunk.start + self.header_size + i * 4, "res type offset")?;
        if offset == NO_ENTRY {
            return Ok(None);
        }
        let pos = self.entries_start + offset as usize;
        let chunk = &data[..self.chunk.end];
        require_len(chunk, pos, 8, "res entry")?;
        let entry_size = read_u16_le(chunk, pos, "res entry")? as usize;
        let flags = read_u16_le(chunk, pos + 2, "res entry")?;
        let total = if flags & 0x0001 != 0 {
            require_len(chunk, pos, 16, "complex res entry")?;
            let count = read_u32_le(chunk, pos + 12, "res entry count")? as usize;
            entry_size.max(16) + count * 12
        } else {
            entry_size.max(8) + 8
        };
        require_len(chunk, pos, total, "res entry")?;
        Ok(Some(&chunk[pos..pos + total]))
    }

    /// Lays the chunk out for writing: verbatim when untouched, otherwise
    /// raw entry bytes copied through with overlay entries encoded once.
    fn plan(&self) -> Result<TypePlan<'_>> {
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
                    serialize_entry(&mut overlay_bytes, entry);
                    EntrySource::Overlay(start..overlay_bytes.len())
                }
                Some(None) => EntrySource::Absent,
                None => match self.raw_entry_bytes(i)? {
                    Some(bytes) => EntrySource::Raw(bytes.len()),
                    None => EntrySource::Absent,
                },
            };
            data_len += match &source {
                EntrySource::Overlay(range) => range.len(),
                EntrySource::Raw(len) => *len,
                EntrySource::Absent => 0,
            };
            entries.push(source);
        }
        let header_size = 20 + config.len();
        let entries_start = header_size + self.len * 4;
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

struct TypePlan<'a> {
    res_type: &'a ResType,
    size: usize,
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

impl TypePlan<'_> {
    fn write(&self, out: &mut dyn Write) -> Result<()> {
        let t = self.res_type;
        let Some(plan) = &self.rebuilt else {
            out.write_all(&t.data.as_bytes()[t.chunk.clone()])?;
            return Ok(());
        };
        let mut head = Vec::with_capacity(plan.entries_start);
        write_u16(&mut head, RES_TABLE_TYPE_TYPE);
        write_u16(&mut head, (20 + plan.config.len()) as u16);
        write_u32(&mut head, self.size as u32);
        head.push(t.id);
        head.push(0);
        write_u16(&mut head, 0);
        write_u32(&mut head, t.len as u32);
        write_u32(&mut head, plan.entries_start as u32);
        head.extend_from_slice(&plan.config);
        let mut offset = 0u32;
        for source in &plan.entries {
            let len = match source {
                EntrySource::Raw(len) => *len,
                EntrySource::Overlay(range) => range.len(),
                EntrySource::Absent => {
                    write_u32(&mut head, NO_ENTRY);
                    continue;
                }
            };
            write_u32(&mut head, offset);
            offset += len as u32;
        }
        out.write_all(&head)?;
        for (i, source) in plan.entries.iter().enumerate() {
            match source {
                EntrySource::Raw(_) => out.write_all(t.raw_entry_bytes(i)?.unwrap())?,
                EntrySource::Overlay(range) => out.write_all(&plan.overlay_bytes[range.clone()])?,
                EntrySource::Absent => {}
            }
        }
        Ok(())
    }
}

impl ResourceTable {
    pub fn parse(data: DexBytes) -> Result<Self> {
        let buf = data.as_bytes();
        require_len(buf, 0, 12, "resource table")?;
        let chunk_type = read_u16_le(buf, 0, "resource table")?;
        if chunk_type != RES_TABLE_TYPE {
            return Err(invalid(
                "resource table",
                format!("expected 0x0002, got 0x{chunk_type:04x}"),
            ));
        }
        let header_size = read_u16_le(buf, 2, "resource table")? as usize;

        let mut global_strings = None;
        let mut packages = Vec::new();
        for (ct, hs, chunk) in chunks(buf, header_size..buf.len(), "resource chunk")? {
            match ct {
                RES_STRING_POOL_TYPE if global_strings.is_none() => {
                    global_strings = Some(ResStringPool::parse(&data, chunk)?);
                }
                RES_TABLE_PACKAGE_TYPE => packages.push(ResPackage::parse(&data, chunk, hs)?),
                _ => {}
            }
        }
        Ok(Self {
            global_strings: global_strings.unwrap_or_else(|| ResStringPool::new(Vec::new(), true)),
            packages,
        })
    }

    pub fn get_string(&self, index: u32) -> Option<Cow<'_, str>> {
        self.global_strings.get(index)
    }

    pub fn set_string(&mut self, index: u32, value: String) {
        self.global_strings.set(index, value);
    }

    /// Adds a string to the global pool and returns its index. Strings added
    /// earlier in this run are reused; the file's own strings are not
    /// searched, since that would index every translation in the table.
    pub fn add_global_string(&mut self, value: &str) -> u32 {
        self.global_strings.intern_added(value)
    }

    /// Find all string-type entries that reference the global string at `string_index`.
    pub fn find_entries_by_string(&self, string_index: u32) -> Vec<ResourceRef> {
        let mut refs = Vec::new();
        for pkg in &self.packages {
            for res_type in &pkg.types {
                for i in 0..res_type.len() {
                    let Some((key, Some((VALUE_TYPE_STRING, data)))) = res_type.entry_head(i) else {
                        continue;
                    };
                    if data == string_index {
                        refs.push(ResourceRef {
                            res_id: res_id(pkg.id, res_type.id, i),
                            package_id: pkg.id,
                            type_id: res_type.id,
                            entry_index: i as u32,
                            key_name: pkg.key_strings.get(key).map(Cow::into_owned).unwrap_or_default(),
                        });
                    }
                }
            }
        }
        refs
    }

    /// Replace a string-type entry's value to point at a different global string index.
    pub fn replace_entry_string(&mut self, res_id: u32, new_string_index: u32) {
        let pkg_id = (res_id >> 24) & 0xFF;
        let type_id = ((res_id >> 16) & 0xFF) as u8;
        let entry_idx = (res_id & 0xFFFF) as usize;
        for res_type in self
            .packages
            .iter_mut()
            .filter(|pkg| pkg.id == pkg_id)
            .flat_map(|pkg| pkg.types.iter_mut())
            .filter(|t| t.id == type_id)
        {
            let Some(mut entry) = res_type.entry(entry_idx) else {
                continue;
            };
            if let ResValue::Simple { data_type: VALUE_TYPE_STRING, data } = &mut entry.value {
                *data = new_string_index;
                res_type.set(entry_idx, Some(entry));
            }
        }
    }

    pub fn contains_resource_id(&self, res_id: u32) -> bool {
        let pkg_id = (res_id >> 24) & 0xFF;
        let type_id = ((res_id >> 16) & 0xFF) as u8;
        let entry_idx = (res_id & 0xFFFF) as usize;
        self.packages.iter().any(|pkg| {
            pkg.id == pkg_id
                && pkg
                    .types
                    .iter()
                    .any(|t| t.id == type_id && t.entry_head(entry_idx).is_some())
        })
    }

    pub fn add_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        data_type: u8,
        data: u32,
    ) -> Option<u32> {
        let pkg = self.packages.first_mut()?;
        let type_id = pkg.ensure_type(type_name)?;
        let key = pkg.key_strings.intern(entry_name);

        let default_type = pkg
            .types
            .iter_mut()
            .find(|t| t.id == type_id && t.is_default_config())?;
        let existing = (0..default_type.len()).find(|&i| {
            default_type.entry_head(i).is_some_and(|(k, _)| k == key)
        });
        let entry = ResEntry {
            flags: 0,
            key,
            value: ResValue::Simple { data_type, data },
        };
        let entry_index = match existing {
            Some(i) => {
                let mut current = default_type.entry(i)?;
                current.value = entry.value;
                default_type.set(i, Some(current));
                i
            }
            None => default_type.push(Some(entry)),
        };

        if let Some(spec) = pkg.type_specs.iter_mut().find(|s| s.id == type_id) {
            while spec.len() <= entry_index {
                spec.push(0);
            }
        }
        for res_type in pkg.types.iter_mut().filter(|t| t.id == type_id) {
            res_type.pad_to(entry_index + 1);
        }
        Some(res_id(pkg.id, type_id, entry_index))
    }

    pub fn add_string_resource(&mut self, name: &str, value: &str) -> Option<u32> {
        let string_idx = self.add_global_string(value);
        self.add_resource("string", name, VALUE_TYPE_STRING, string_idx)
    }

    pub fn add_bool_resource(&mut self, name: &str, value: bool) -> Option<u32> {
        self.add_resource("bool", name, 0x12, if value { 0xFFFF_FFFF } else { 0 })
    }

    pub fn add_integer_resource(&mut self, name: &str, value: i32) -> Option<u32> {
        self.add_resource("integer", name, 0x10, value as u32)
    }

    pub fn add_color_resource(&mut self, name: &str, argb: u32) -> Option<u32> {
        self.add_resource("color", name, 0x1c, argb)
    }

    pub fn add_dimen_resource(&mut self, name: &str, encoded_dim: u32) -> Option<u32> {
        self.add_resource("dimen", name, 0x05, encoded_dim)
    }

    pub fn ensure_id(&mut self, name: &str) -> Option<u32> {
        if let Some(existing) = self.find_resource_id("id", name) {
            return Some(existing);
        }
        self.add_resource("id", name, 0x01, 0)
    }

    pub fn resource_exists(&self, type_name: &str, entry_name: &str) -> bool {
        self.find_entry(type_name, entry_name).is_some()
    }

    pub fn get_string_value(&self, name: &str) -> Option<Cow<'_, str>> {
        match self.find_entry("string", name)?.3 {
            Some((VALUE_TYPE_STRING, data)) => self.get_string(data),
            _ => None,
        }
    }

    pub fn set_string_value(&mut self, name: &str, value: &str) -> bool {
        match self.find_entry("string", name).map(|found| found.3) {
            Some(Some((VALUE_TYPE_STRING, data))) => {
                self.set_string(data, value.to_string());
                true
            }
            _ => false,
        }
    }

    pub fn add_color_parsed(&mut self, name: &str, color: &str) -> Option<u32> {
        let (data_type, data) = crate::axml::compiler::parse_color(color)?;
        self.add_resource("color", name, data_type, data)
    }

    pub fn add_dimen_parsed(&mut self, name: &str, dimen: &str) -> Option<u32> {
        let encoded = crate::axml::compiler::parse_dimension(dimen)?;
        self.add_resource("dimen", name, 0x05, encoded)
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.find_entry(type_name, entry_name)
            .map(|(pkg_id, type_id, i, _)| res_id(pkg_id, type_id, i))
    }

    pub fn get_resource_value(&self, type_name: &str, entry_name: &str) -> Option<(u8, u32)> {
        self.find_entry(type_name, entry_name)?.3
    }

    /// The first entry named `entry_name` in the type named `type_name`.
    fn find_entry(&self, type_name: &str, entry_name: &str) -> Option<FoundEntry> {
        for pkg in &self.packages {
            let Some(type_id) = pkg.type_strings.find(type_name).and_then(|i| u8::try_from(i + 1).ok()) else {
                continue;
            };
            let Some(key) = pkg.key_strings.find(entry_name) else {
                continue;
            };
            for res_type in pkg.types.iter().filter(|t| t.id == type_id) {
                for i in 0..res_type.len() {
                    if let Some((k, simple)) = res_type.entry_head(i) {
                        if k == key {
                            return Some((pkg.id, type_id, i, simple));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_to(&mut out)?;
        Ok(out)
    }

    /// Writes the table into an unlinked temp file, so a table of any size
    /// costs no heap beyond the entries a patch added or changed.
    pub fn serialize_spooled(&self) -> Result<File> {
        let mut file = tempfile::tempfile()?;
        let mut out = BufWriter::with_capacity(1 << 20, &mut file);
        self.write_to(&mut out)?;
        out.flush()?;
        drop(out);
        Ok(file)
    }

    pub fn write_to(&self, out: &mut dyn Write) -> Result<()> {
        let global = self.global_strings.plan();
        let packages = self
            .packages
            .iter()
            .map(ResPackage::plan)
            .collect::<Result<Vec<_>>>()?;
        let total = 12 + global.size + packages.iter().map(|p| p.size).sum::<usize>();
        let mut head = Vec::with_capacity(12);
        write_u16(&mut head, RES_TABLE_TYPE);
        write_u16(&mut head, 12);
        write_u32(&mut head, total as u32);
        write_u32(&mut head, self.packages.len() as u32);
        out.write_all(&head)?;
        global.write(out)?;
        for pkg in &packages {
            pkg.write(out)?;
        }
        Ok(())
    }
}

impl ResPackage {
    pub fn new(id: u32, name: &str, type_strings: ResStringPool, key_strings: ResStringPool) -> Self {
        Self {
            id,
            name: name.to_string(),
            type_strings,
            key_strings,
            last_public_type: 0,
            last_public_key: 0,
            type_id_offset: 0,
            type_specs: Vec::new(),
            types: Vec::new(),
        }
    }

    fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(32), "resource package")?;
        let id = read_u32_le(buf, 8, "resource package")?;
        let name = {
            let mut units = Vec::new();
            let mut p = 12;
            while p + 2 <= (12 + 256).min(buf.len()) {
                let cu = read_u16_le(buf, p, "package name")?;
                if cu == 0 {
                    break;
                }
                units.push(cu);
                p += 2;
            }
            String::from_utf16(&units).unwrap_or_default()
        };
        let type_strings_offset = read_u32_le(buf, 268, "resource package")? as usize;
        let last_public_type = read_u32_le(buf, 272, "resource package")?;
        let key_strings_offset = read_u32_le(buf, 276, "resource package")? as usize;
        let last_public_key = read_u32_le(buf, 280, "resource package")?;
        let type_id_offset = if header_size >= 288 {
            read_u32_le(buf, 284, "resource package")?
        } else {
            0
        };

        let pool = |offset: usize, what: &'static str| -> Result<ResStringPool> {
            if offset == 0 {
                return Ok(ResStringPool::new(Vec::new(), true));
            }
            if offset >= buf.len() {
                return Err(malformed("resource package", offset, what));
            }
            let end = find_chunk_end(buf, offset)?;
            ResStringPool::parse(data, chunk.start + offset..chunk.start + end)
        };
        let type_strings = pool(type_strings_offset, "type string pool offset is outside package")?;
        let key_strings = pool(key_strings_offset, "key string pool offset is outside package")?;

        let body_start = if key_strings_offset > 0 {
            find_chunk_end(buf, key_strings_offset)?
        } else if type_strings_offset > 0 {
            find_chunk_end(buf, type_strings_offset)?
        } else {
            header_size
        };

        let mut type_specs = Vec::new();
        let mut types = Vec::new();
        for (ct, hs, sub) in chunks(buf, body_start..buf.len(), "package chunk")? {
            let sub = chunk.start + sub.start..chunk.start + sub.end;
            match ct {
                RES_TABLE_TYPE_SPEC => type_specs.push(TypeSpec::parse(data, sub, hs)?),
                RES_TABLE_TYPE_TYPE => types.push(ResType::parse(data, sub, hs)?),
                _ => {}
            }
        }

        Ok(Self {
            id,
            name,
            type_strings,
            key_strings,
            last_public_type,
            last_public_key,
            type_id_offset,
            type_specs,
            types,
        })
    }

    pub fn ensure_type(&mut self, type_name: &str) -> Option<u8> {
        if let Some(pos) = self.type_strings.find(type_name) {
            return u8::try_from(pos + 1).ok();
        }
        if self.type_strings.len() >= u8::MAX as usize {
            return None;
        }
        self.type_strings.push(type_name);
        let type_id = self.type_strings.len() as u8;
        self.type_specs.push(TypeSpec::new(type_id, Vec::new()));
        self.types.push(ResType::new(type_id, Vec::new()));
        Some(type_id)
    }

    fn plan(&self) -> Result<PackagePlan<'_>> {
        let type_strings = self.type_strings.plan();
        let key_strings = self.key_strings.plan();
        let types = self.types.iter().map(ResType::plan).collect::<Result<Vec<_>>>()?;
        let size = 288
            + type_strings.size
            + key_strings.size
            + self.type_specs.iter().map(TypeSpec::size).sum::<usize>()
            + types.iter().map(|t| t.size).sum::<usize>();
        Ok(PackagePlan {
            package: self,
            size,
            type_strings,
            key_strings,
            types,
        })
    }
}

struct PackagePlan<'a> {
    package: &'a ResPackage,
    size: usize,
    type_strings: PoolPlan<'a>,
    key_strings: PoolPlan<'a>,
    types: Vec<TypePlan<'a>>,
}

impl PackagePlan<'_> {
    fn write(&self, out: &mut dyn Write) -> Result<()> {
        let pkg = self.package;
        let header_size: usize = 288;
        let mut head = Vec::with_capacity(header_size);
        write_u16(&mut head, RES_TABLE_PACKAGE_TYPE);
        write_u16(&mut head, header_size as u16);
        write_u32(&mut head, self.size as u32);
        write_u32(&mut head, pkg.id);
        let name_units: Vec<u16> = pkg.name.encode_utf16().collect();
        for i in 0..128 {
            write_u16(&mut head, name_units.get(i).copied().unwrap_or(0));
        }
        write_u32(&mut head, header_size as u32);
        write_u32(&mut head, pkg.last_public_type);
        write_u32(&mut head, (header_size + self.type_strings.size) as u32);
        write_u32(&mut head, pkg.last_public_key);
        write_u32(&mut head, pkg.type_id_offset);
        out.write_all(&head)?;
        self.type_strings.write(out)?;
        self.key_strings.write(out)?;
        for spec in &pkg.type_specs {
            spec.write(out)?;
        }
        for t in &self.types {
            t.write(out)?;
        }
        Ok(())
    }
}

fn res_id(package_id: u32, type_id: u8, entry_index: usize) -> u32 {
    (package_id << 24) | ((type_id as u32) << 16) | (entry_index as u32)
}

/// `(chunk type, header size, chunk range)` of one chunk in the table.
type ChunkHeader = (u16, usize, Range<usize>);

/// Walks the chunk headers in `range` of `buf`.
fn chunks(buf: &[u8], range: Range<usize>, what: &'static str) -> Result<Vec<ChunkHeader>> {
    let mut out = Vec::new();
    let mut pos = range.start;
    while pos + 8 <= range.end {
        let ct = read_u16_le(buf, pos, what)?;
        let hs = read_u16_le(buf, pos + 2, what)? as usize;
        let cs = read_u32_le(buf, pos + 4, what)? as usize;
        if cs < 8 || hs < 8 || hs > cs || pos + cs > range.end {
            return Err(malformed(what, pos, "chunk extends past end of table"));
        }
        out.push((ct, hs, pos..pos + cs));
        pos += cs;
    }
    Ok(out)
}

fn find_chunk_end(data: &[u8], offset: usize) -> Result<usize> {
    require_len(data, offset, 8, "chunk header")?;
    let cs = read_u32_le(data, offset + 4, "chunk size")? as usize;
    if cs < 8 || offset + cs > data.len() {
        return Err(malformed(
            "chunk header",
            offset,
            "chunk extends past end of containing buffer",
        ));
    }
    Ok(offset + cs)
}

fn parse_entry(bytes: &[u8]) -> Result<ResEntry> {
    let flags = read_u16_le(bytes, 2, "res entry")?;
    let key = read_u32_le(bytes, 4, "res entry")?;
    let value = if flags & 0x0001 != 0 {
        let parent = read_u32_le(bytes, 8, "res entry parent")?;
        let count = read_u32_le(bytes, 12, "res entry count")? as usize;
        let entries = (0..count)
            .map(|j| {
                let pos = 16 + j * 12;
                Ok(MapEntry {
                    name: read_u32_le(bytes, pos, "map entry")?,
                    data_type: bytes[pos + 7],
                    data: read_u32_le(bytes, pos + 8, "map entry")?,
                })
            })
            .collect::<Result<_>>()?;
        ResValue::Complex { parent, entries }
    } else {
        ResValue::Simple {
            data_type: bytes[11],
            data: read_u32_le(bytes, 12, "res value")?,
        }
    };
    Ok(ResEntry { flags, key, value })
}

fn serialize_entry(out: &mut Vec<u8>, entry: &ResEntry) {
    match &entry.value {
        ResValue::Simple { data_type, data } => {
            write_u16(out, 8);
            write_u16(out, entry.flags);
            write_u32(out, entry.key);
            write_u16(out, 8);
            out.push(0);
            out.push(*data_type);
            write_u32(out, *data);
        }
        ResValue::Complex { parent, entries } => {
            write_u16(out, 16);
            write_u16(out, entry.flags | 0x0001);
            write_u32(out, entry.key);
            write_u32(out, *parent);
            write_u32(out, entries.len() as u32);
            for me in entries {
                write_u32(out, me.name);
                write_u16(out, 8);
                out.push(0);
                out.push(me.data_type);
                write_u32(out, me.data);
            }
        }
    }
}

// Resource string pools encode supplementary characters as surrogate pairs
// (MUTF-8), unlike AXML which uses standard UTF-8.
fn decode_res_utf8(data: &[u8], offset: usize) -> Result<Cow<'_, str>> {
    let mut pos = offset;
    if pos >= data.len() {
        return Ok(Cow::Borrowed(""));
    }
    pos += if data[pos] & 0x80 != 0 { 2 } else { 1 };
    if pos >= data.len() {
        return Ok(Cow::Borrowed(""));
    }
    let byte_len = if data[pos] & 0x80 != 0 {
        let hi = (data[pos] & 0x7F) as usize;
        pos += 1;
        if pos >= data.len() {
            return Ok(Cow::Borrowed(""));
        }
        let lo = data[pos] as usize;
        pos += 1;
        (hi << 8) | lo
    } else {
        let len = data[pos] as usize;
        pos += 1;
        len
    };
    if pos + byte_len > data.len() {
        return Err(malformed("res utf8 string", pos, "string extends past pool"));
    }
    let bytes = &data[pos..pos + byte_len];
    match std::str::from_utf8(bytes) {
        Ok(s) if !bytes.contains(&0xED) => Ok(Cow::Borrowed(s)),
        _ => reseam_dex::encoding::mutf8::decode_mutf8(bytes)
            .map(Cow::Owned)
            .map_err(|_| invalid("res utf8 string", "invalid UTF-8/MUTF-8")),
    }
}

fn decode_res_utf16(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;
    if pos + 2 > data.len() {
        return Ok(String::new());
    }
    let first = read_u16_le(data, pos, "res utf16 length")?;
    let char_count = if first & 0x8000 != 0 {
        let next = read_u16_le(data, pos + 2, "res utf16 length")? as usize;
        pos += 4;
        (((first & 0x7FFF) as usize) << 16) | next
    } else {
        pos += 2;
        first as usize
    };
    if char_count > MAX_UTF16_CODE_UNITS {
        return Err(invalid("res utf16 string", "string length exceeds safety limit"));
    }
    if pos + char_count * 2 > data.len() {
        return Err(malformed("res utf16 string", pos, "string extends past pool"));
    }
    let units: Vec<u16> = (0..char_count)
        .map(|i| read_u16_le(data, pos + i * 2, "res utf16 string"))
        .collect::<Result<_>>()?;
    String::from_utf16(&units).map_err(|_| invalid("res utf16 string", "invalid UTF-16"))
}

fn hash_str(s: &str) -> u32 {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish() as u32
}
