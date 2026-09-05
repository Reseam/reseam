// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Android binary string pools (`ResStringPool_header`), shared by AXML and
//! `resources.arsc`. Strings are read in place; strings added after parse are
//! owned and edits are overrides. Style spans stay in the file and are copied
//! through on write, and appended strings carry none.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::ops::Range;
use std::sync::OnceLock;

use reseam_dex::file::DexBytes;
use rustc_hash::FxHasher;

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::{invalid, malformed, Result};

pub(crate) const CHUNK_STRING_POOL: u16 = 0x0001;
const HEADER_LEN: usize = 28;
const FLAG_UTF8: u32 = 1 << 8;
const MAX_STRINGS: usize = 1_000_000;
const MAX_UTF16_CODE_UNITS: usize = 1_000_000;

#[derive(Debug, Clone, Default)]
pub struct StringPool {
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
    /// `(hash, index)` of every raw string, sorted by hash, built on first lookup.
    index: OnceLock<Vec<(u32, u32)>>,
}

impl StringPool {
    pub fn new(strings: Vec<String>, is_utf8: bool) -> Self {
        Self {
            is_utf8,
            owned: strings,
            ..Self::default()
        }
    }

    pub(crate) fn parse(data: &DexBytes, chunk: Range<usize>) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, HEADER_LEN, "string pool")?;
        let header_size = read_u16_le(buf, 2, "string pool")? as usize;
        let string_count = read_u32_le(buf, 8, "string pool")? as usize;
        if string_count > MAX_STRINGS {
            return Err(invalid("string pool", "string count exceeds safety limit"));
        }
        let style_count = read_u32_le(buf, 12, "string pool")? as usize;
        let flags = read_u32_le(buf, 16, "string pool")?;
        let strings_start = read_u32_le(buf, 20, "string pool")? as usize;
        let styles_start = read_u32_le(buf, 24, "string pool")? as usize;
        require_len(
            buf,
            header_size,
            (string_count + style_count) * 4,
            "string pool offsets",
        )?;
        let styles_in_range = styles_start >= strings_start && styles_start <= buf.len();
        if strings_start > buf.len() || (style_count > 0 && !styles_in_range) {
            return Err(malformed(
                "string pool",
                20,
                "string or style data start is outside pool",
            ));
        }

        let pool = Self {
            data: data.clone(),
            chunk: chunk.clone(),
            raw_len: string_count,
            style_count,
            is_utf8: flags & FLAG_UTF8 != 0,
            offsets_start: chunk.start + header_size,
            strings_start: chunk.start + strings_start,
            styles_start: chunk.start
                + if style_count > 0 {
                    styles_start
                } else {
                    buf.len()
                },
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
        self.owned
            .get(i - self.raw_len)
            .map(|s| Cow::Borrowed(s.as_str()))
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
    pub(crate) fn intern_added(&mut self, value: &str) -> u32 {
        self.find_added(value).unwrap_or_else(|| self.push(value))
    }

    fn raw(&self, i: usize) -> Result<Cow<'_, str>> {
        let data = self.data.as_bytes();
        let offset = read_u32_le(data, self.offsets_start + i * 4, "string offset")? as usize;
        let abs = self.strings_start + offset;
        if abs >= self.styles_start {
            return Err(malformed(
                "string pool",
                abs,
                "string offset extends past pool",
            ));
        }
        let pool = &data[..self.styles_start];
        if self.is_utf8 {
            decode_utf8(pool, abs)
        } else {
            decode_utf16(pool, abs).map(Cow::Owned)
        }
    }

    /// Lays the pool out for writing: verbatim when untouched, otherwise the
    /// raw string bytes and style tables copied through with appended and
    /// overridden strings encoded after them.
    pub(crate) fn plan(&self) -> PoolPlan<'_> {
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
                offsets.push(
                    read_u32_le(data, self.offsets_start + i as usize * 4, "string offset")
                        .unwrap_or(0),
                );
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
            let padded = (raw_region.len() + extra.len()).div_ceil(4) * 4;
            extra.resize(padded - raw_region.len(), 0);
        }
        let strings_start = HEADER_LEN + (count + self.style_count) * 4;
        let styles_start = strings_start + raw_region.len() + extra.len();
        let style_data = self.styles_start..self.chunk.end;
        PoolPlan {
            pool: self,
            size: (styles_start + style_data.len() + 3) & !3,
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
}

pub(crate) struct PoolPlan<'a> {
    pool: &'a StringPool,
    pub size: usize,
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
    pub(crate) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let pool = self.pool;
        let data = pool.data.as_bytes();
        let Some(plan) = &self.rebuilt else {
            out.write_all(&data[pool.chunk.clone()])?;
            return Ok(());
        };
        let mut header = Vec::with_capacity(HEADER_LEN + plan.offsets.len() * 4);
        write_u16(&mut header, CHUNK_STRING_POOL);
        write_u16(&mut header, HEADER_LEN as u16);
        write_u32(&mut header, self.size as u32);
        write_u32(&mut header, plan.offsets.len() as u32);
        write_u32(&mut header, pool.style_count as u32);
        write_u32(&mut header, if pool.is_utf8 { FLAG_UTF8 } else { 0 });
        write_u32(&mut header, plan.strings_start as u32);
        write_u32(&mut header, plan.styles_start as u32);
        for offset in &plan.offsets {
            write_u32(&mut header, *offset);
        }
        out.write_all(&header)?;
        let style_offsets = pool.offsets_start + pool.raw_len * 4;
        out.write_all(&data[style_offsets..style_offsets + pool.style_count * 4])?;
        out.write_all(&data[plan.raw_region.clone()])?;
        out.write_all(&plan.extra)?;
        out.write_all(&data[plan.style_data.clone()])?;
        let written = header.len()
            + pool.style_count * 4
            + plan.raw_region.len()
            + plan.extra.len()
            + plan.style_data.len();
        out.write_all(&vec![0u8; self.size - written])?;
        Ok(())
    }
}

/// Resource string pools encode supplementary characters as surrogate pairs
/// (MUTF-8), which standard UTF-8 rejects.
fn decode_utf8(data: &[u8], offset: usize) -> Result<Cow<'_, str>> {
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
        return Err(malformed("utf8 string", pos, "string extends past pool"));
    }
    let bytes = &data[pos..pos + byte_len];
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(Cow::Borrowed(s)),
        Err(_) => reseam_dex::encoding::mutf8::decode_mutf8(bytes)
            .map(Cow::Owned)
            .map_err(|_| invalid("utf8 string", "invalid UTF-8/MUTF-8")),
    }
}

fn decode_utf16(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;
    if pos + 2 > data.len() {
        return Ok(String::new());
    }
    let first = read_u16_le(data, pos, "utf16 length")?;
    let char_count = if first & 0x8000 != 0 {
        let next = read_u16_le(data, pos + 2, "utf16 length")? as usize;
        pos += 4;
        (((first & 0x7FFF) as usize) << 16) | next
    } else {
        pos += 2;
        first as usize
    };
    if char_count > MAX_UTF16_CODE_UNITS {
        return Err(invalid(
            "utf16 string",
            "string length exceeds safety limit",
        ));
    }
    if pos + char_count * 2 > data.len() {
        return Err(malformed("utf16 string", pos, "string extends past pool"));
    }
    let units: Vec<u16> = (0..char_count)
        .map(|i| read_u16_le(data, pos + i * 2, "utf16 string"))
        .collect::<Result<_>>()?;
    String::from_utf16(&units).map_err(|_| invalid("utf16 string", "invalid UTF-16"))
}

fn encode_utf8(out: &mut Vec<u8>, s: &str) {
    encode_len_u8(out, s.chars().count());
    encode_len_u8(out, s.len());
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn encode_len_u8(out: &mut Vec<u8>, len: usize) {
    if len > 0x7F {
        out.push(((len >> 8) & 0x7F) as u8 | 0x80);
    }
    out.push((len & 0xFF) as u8);
}

fn encode_utf16(out: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() > 0x7FFF {
        write_u16(out, ((units.len() >> 16) as u16) | 0x8000);
    }
    write_u16(out, (units.len() & 0xFFFF) as u16);
    for unit in &units {
        write_u16(out, *unit);
    }
    write_u16(out, 0);
}

fn hash_str(s: &str) -> u32 {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish() as u32
}
