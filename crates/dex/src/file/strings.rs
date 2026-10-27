// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::u32_at;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;

mod lookup;

use super::sorted_index::SortedIndex;
use crate::DexBytes;
use crate::encoding::leb128::{read_uleb128, write_uleb128};
use crate::encoding::mutf8::{ascii_order, mutf8_compare, mutf8_units};
use crate::encoding::mutf8::{decode_mutf8_lossy, encode_mutf8, utf16_len, utf16_units};
use crate::error::{Result, invalid, invalid_mutf8};
use crate::model::StringIdx;
use crate::options::ParseOptions;

/// The string table left in the file: raw entries are read through the
/// `string_ids` table on access, and only strings added after parse are owned.
#[derive(Debug, Clone, Default)]
pub struct StringPool {
    raw: Option<DexBytes>,
    ids_off: usize,
    raw_len: usize,
    owned: Vec<Box<str>>,
    index: SortedIndex,
}

impl StringPool {
    pub(crate) fn from_raw(
        raw: DexBytes,
        ids_off: u32,
        count: u32,
        opts: ParseOptions,
    ) -> Result<Self> {
        let buf = raw.as_bytes();
        let ids_off = ids_off as usize;
        let count = count as usize;
        crate::error::require_array(buf, ids_off, count, 4, "string IDs")?;
        for i in 0..count {
            validate_item(buf, u32_at(buf, ids_off + i * 4), opts)?;
        }
        let mut pool = Self {
            raw: Some(raw),
            ids_off,
            raw_len: count,
            ..Self::default()
        };
        pool.index = SortedIndex::new(
            count,
            |i| pool.compare(i as u32 - 1, i as u32) == Ordering::Less,
            |i| hash_units(mutf8_units(&pool.mutf8(i))),
        );
        Ok(pool)
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.owned.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, idx: StringIdx) -> Cow<'_, str> {
        let i = idx.0 as usize;
        if i < self.raw_len {
            let payload = self.payload(self.raw_offset(i));
            match std::str::from_utf8(payload) {
                Ok(s) => Cow::Borrowed(s),
                Err(_) => Cow::Owned(decode_mutf8_lossy(payload)),
            }
        } else {
            Cow::Borrowed(&self.owned[i - self.raw_len])
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Cow<'_, str>> {
        (0..self.len()).map(|i| self.get(StringIdx(i as u32)))
    }

    pub fn intern(&mut self, s: &str) -> StringIdx {
        self.find(s).unwrap_or_else(|| self.push(s))
    }

    pub fn push(&mut self, s: &str) -> StringIdx {
        let idx = StringIdx(self.len() as u32);
        self.owned.push(s.into());
        self.index.push(hash_str(s), idx.0 as usize);
        idx
    }

    /// Drops the strings pushed after the pool held `len`.
    pub(crate) fn truncate(&mut self, len: usize) {
        assert!(len >= self.raw_len, "only pushed strings are truncated");
        self.index
            .truncate(len..self.len(), |i| hash_str(&self.owned[i - self.raw_len]));
        self.owned.truncate(len - self.raw_len);
    }

    pub(crate) fn compare(&self, a: u32, b: u32) -> Ordering {
        let ascii = self
            .raw_payload(a as usize)
            .zip(self.raw_payload(b as usize))
            .and_then(|(x, y)| ascii_order(x, y));
        if let Some(order) = ascii {
            return order;
        }
        // A raw payload that is valid UTF-8 holds only U+0001..U+FFFF (NUL and
        // supplementary characters have non-UTF-8 encodings in MUTF-8), and for
        // that range byte order equals UTF-16 code unit order. Otherwise compare
        // the MUTF-8 payloads directly: a scalar decode drops surrogate halves to
        // U+FFFD, which would order an entry differently from the bytes written.
        match (self.plain(a as usize), self.plain(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => mutf8_compare(&self.mutf8(a as usize), &self.mutf8(b as usize)),
        }
    }

    fn mutf8(&self, i: usize) -> Cow<'_, [u8]> {
        if i < self.raw_len {
            Cow::Borrowed(self.payload(self.raw_offset(i)))
        } else {
            Cow::Owned(encode_mutf8(&self.owned[i - self.raw_len]))
        }
    }

    pub(crate) fn item(&self, idx: StringIdx) -> Cow<'_, [u8]> {
        let i = idx.0 as usize;
        if i < self.raw_len {
            let off = self.raw_offset(i);
            let end = off as usize + self.payload_range(off).1;
            Cow::Borrowed(&self.raw_bytes()[off as usize..=end])
        } else {
            let s = &self.owned[i - self.raw_len];
            let mut out = Vec::with_capacity(s.len() + 6);
            write_uleb128(&mut out, utf16_len(s));
            out.extend_from_slice(&encode_mutf8(s));
            out.push(0);
            Cow::Owned(out)
        }
    }

    /// How many leading entries are in DEX sort order.
    pub(crate) fn sorted_len(&self) -> usize {
        self.index.sorted_len()
    }

    /// Whether every entry is already in DEX sort order.
    pub fn is_sorted(&self) -> bool {
        self.index.sorted_len() == self.len()
    }

    pub fn heap_bytes(&self) -> u64 {
        let owned: usize = self
            .owned
            .iter()
            .map(|s| s.len() + size_of::<Box<str>>())
            .sum();
        (owned + self.index.heap_bytes()) as u64
    }

    fn plain(&self, i: usize) -> Option<&[u8]> {
        if i >= self.raw_len {
            return None;
        }
        let bytes = self.payload(self.raw_offset(i));
        std::str::from_utf8(bytes).ok().map(str::as_bytes)
    }

    fn raw_offset(&self, i: usize) -> u32 {
        let buf = self.raw_bytes();
        let o = self.ids_off + i * 4;
        u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
    }

    fn raw_bytes(&self) -> &[u8] {
        self.raw
            .as_ref()
            .expect("raw entries exist only while the buffer is retained")
            .as_bytes()
    }

    fn payload(&self, off: u32) -> &[u8] {
        let (start, end) = self.payload_range(off);
        &self.raw_bytes()[off as usize + start..off as usize + end]
    }

    /// A raw entry's payload through the end of the buffer, so its terminator
    /// ends it; `None` for an owned entry.
    fn raw_payload(&self, i: usize) -> Option<&[u8]> {
        (i < self.raw_len).then(|| {
            let item = &self.raw_bytes()[self.raw_offset(i) as usize..];
            &item[payload_start(item)..]
        })
    }

    fn payload_range(&self, off: u32) -> (usize, usize) {
        let item = &self.raw_bytes()[off as usize..];
        let start = payload_start(item);
        let end = start
            + item[start..]
                .iter()
                .position(|&b| b == 0)
                .expect("raw string framing validated its terminator");
        (start, end)
    }
}

impl<S: Into<Box<str>>> FromIterator<S> for StringPool {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        let mut pool = Self::default();
        for s in iter {
            let s: Box<str> = s.into();
            pool.push(&s);
        }
        pool
    }
}

/// Where an item's payload starts, past its ULEB128 length.
fn payload_start(item: &[u8]) -> usize {
    1 + item.iter().take_while(|&&b| b & 0x80 != 0).count()
}

fn validate_item(buf: &[u8], off: u32, opts: ParseOptions) -> Result<()> {
    let (declared, leb_size) = read_uleb128(buf, off as usize, opts)?;
    let start = off as usize + leb_size;
    let rest = buf
        .get(start..)
        .ok_or_else(|| invalid_mutf8(start, "string data past end of buffer"))?;
    let len = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| invalid_mutf8(start, "missing NUL terminator"))?;
    let payload = &rest[..len];
    let actual = if payload.is_ascii() {
        len as u32
    } else {
        utf16_units(payload, start, opts)?
    };
    if actual != declared {
        return Err(invalid(
            "string data",
            format!("declared UTF-16 length {declared} does not match decoded length {actual}"),
        ));
    }
    Ok(())
}

fn hash_str(s: &str) -> u64 {
    hash_units(s.encode_utf16())
}

fn hash_units(units: impl Iterator<Item = u16>) -> u64 {
    let mut hasher = FxHasher::default();
    for unit in units {
        unit.hash(&mut hasher);
    }
    hasher.finish()
}
