// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::DexBytes;
use crate::encoding::leb128::{read_uleb128_with_opts, write_uleb128};
use crate::encoding::mutf8::{decode_mutf8_lossy, encode_mutf8, utf16_len, utf16_units};
use crate::error::{invalid, invalid_mutf8, invalid_offset, Result};
use crate::read::header::u32_at;
use crate::types::header::ParseOptions;
use crate::types::StringIdx;
use crate::util::sort::dex_string_compare;

/// The string table left in the file: raw entries are read through the
/// `string_ids` table on access, and only strings added after parse are owned.
///
/// The leading `sorted_len` entries are in DEX sort order (the format requires
/// it), so lookups binary-search them; entries after that are found through a
/// small hash index.
#[derive(Debug, Clone, Default)]
pub struct StringPool {
    raw: Option<DexBytes>,
    ids_off: usize,
    raw_len: usize,
    owned: Vec<Box<str>>,
    sorted_len: usize,
    tail: FxHashMap<u64, SmallVec<[u32; 1]>>,
}

impl StringPool {
    pub(crate) fn from_raw(
        raw: DexBytes,
        ids_off: u32,
        count: u32,
        opts: &ParseOptions,
    ) -> Result<Self> {
        let buf = raw.as_bytes();
        let ids_off = ids_off as usize;
        let count = count as usize;
        if ids_off + count * 4 > buf.len() {
            return Err(invalid_offset(
                "string_ids",
                ids_off as u32,
                buf.len() as u32,
            ));
        }
        for i in 0..count {
            validate_item(buf, u32_at(buf, ids_off + i * 4)?, opts)?;
        }
        let mut pool = Self {
            raw: Some(raw),
            ids_off,
            raw_len: count,
            ..Self::default()
        };
        pool.rebuild_tail();
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

    pub fn find(&self, s: &str) -> Option<StringIdx> {
        let bmp = s.chars().all(|c| (c as u32) < 0x10000);
        let mut lo = 0usize;
        let mut hi = self.sorted_len;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let ord = match self.plain(mid) {
                Some(bytes) if bmp => bytes.cmp(s.as_bytes()),
                _ => dex_string_compare(&self.get(StringIdx(mid as u32)), s),
            };
            match ord {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(StringIdx(mid as u32)),
            }
        }
        self.tail
            .get(&hash_str(s))?
            .iter()
            .copied()
            .find(|&i| self.get(StringIdx(i)) == s)
            .map(StringIdx)
    }

    pub fn intern(&mut self, s: &str) -> StringIdx {
        self.find(s).unwrap_or_else(|| self.push(s))
    }

    pub fn push(&mut self, s: &str) -> StringIdx {
        let idx = StringIdx(self.len() as u32);
        self.owned.push(s.into());
        self.tail.entry(hash_str(s)).or_default().push(idx.0);
        idx
    }

    /// Ordering of two entries under DEX string sort order.
    pub(crate) fn compare(&self, a: u32, b: u32) -> Ordering {
        // A raw payload that is valid UTF-8 holds only U+0001..U+FFFF (NUL and
        // supplementary characters have non-UTF-8 encodings in MUTF-8), and for
        // that range byte order equals UTF-16 code unit order.
        match (self.plain(a as usize), self.plain(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => dex_string_compare(&self.get(StringIdx(a)), &self.get(StringIdx(b))),
        }
    }

    /// Keeps only the entries in `keep`, in index order, as owned strings.
    pub(crate) fn retain(&mut self, keep: &HashSet<u32>) {
        let owned: Vec<Box<str>> = (0..self.len() as u32)
            .filter(|i| keep.contains(i))
            .map(|i| self.get(StringIdx(i)).into())
            .collect();
        *self = Self::from_iter(owned);
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        if len >= self.raw_len {
            self.owned.truncate(len - self.raw_len);
        } else {
            self.raw_len = len;
            self.owned.clear();
        }
        self.rebuild_tail();
    }

    /// The string_data_item bytes for `idx`; raw entries are served verbatim.
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

    /// Whether every entry is already in DEX sort order.
    pub fn is_sorted(&self) -> bool {
        self.sorted_len == self.len()
    }

    pub fn heap_bytes(&self) -> u64 {
        let owned: usize = self
            .owned
            .iter()
            .map(|s| s.len() + size_of::<Box<str>>())
            .sum();
        (owned + self.tail.len() * 24) as u64
    }

    fn rebuild_tail(&mut self) {
        let len = self.len();
        self.sorted_len = (1..len)
            .find(|&i| self.compare(i as u32 - 1, i as u32) != Ordering::Less)
            .unwrap_or(len);
        let mut tail: FxHashMap<u64, SmallVec<[u32; 1]>> = FxHashMap::default();
        for i in self.sorted_len..len {
            let h = hash_str(&self.get(StringIdx(i as u32)));
            tail.entry(h).or_default().push(i as u32);
        }
        self.tail = tail;
    }

    /// The payload of a raw entry when it is valid UTF-8, i.e. BMP-only.
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

    /// `(payload start, NUL terminator)` relative to a validated item offset.
    fn payload_range(&self, off: u32) -> (usize, usize) {
        let item = &self.raw_bytes()[off as usize..];
        let start = 1 + item.iter().take_while(|&&b| b & 0x80 != 0).count();
        let end = start + item[start..].iter().position(|&b| b == 0).unwrap();
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

fn validate_item(buf: &[u8], off: u32, opts: &ParseOptions) -> Result<()> {
    let (declared, leb_size) = read_uleb128_with_opts(buf, off as usize, opts)?;
    let start = off as usize + leb_size;
    let rest = buf
        .get(start..)
        .ok_or_else(|| invalid_mutf8(start, "string data past end of buffer"))?;
    let len = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| invalid_mutf8(start, "missing NUL terminator"))?;
    let actual = utf16_units(&rest[..len], start, opts)?;
    if actual != declared {
        return Err(invalid(
            "string data",
            format!("declared UTF-16 length {declared} does not match decoded length {actual}"),
        ));
    }
    Ok(())
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        write_uleb128(&mut out, utf16_len(s));
        out.extend_from_slice(&encode_mutf8(s));
        out.push(0);
        out
    }

    fn raw_pool(strings: &[&str]) -> StringPool {
        let mut buf = Vec::new();
        let mut offsets = Vec::new();
        for s in strings {
            offsets.push(buf.len() as u32);
            buf.extend_from_slice(&item(s));
        }
        let ids_off = buf.len() as u32;
        for off in offsets {
            buf.extend_from_slice(&off.to_le_bytes());
        }
        StringPool::from_raw(
            DexBytes::from_vec(buf),
            ids_off,
            strings.len() as u32,
            &ParseOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn raw_entries_borrow_and_decode() {
        let pool = raw_pool(&["", "a", "b\0c", "é", "\u{1F600}"]);
        assert!(matches!(pool.get(StringIdx(1)), Cow::Borrowed("a")));
        assert!(matches!(pool.get(StringIdx(3)), Cow::Borrowed("é")));
        assert_eq!(pool.get(StringIdx(2)), "b\0c");
        assert_eq!(pool.get(StringIdx(4)), "\u{1F600}");
    }

    #[test]
    fn sorted_lookup_and_tail_lookup() {
        let mut pool = raw_pool(&["", "a", "b\0c", "é", "\u{1F600}", "\u{FFFF}", "0"]);
        assert_eq!(pool.sorted_len, 6);
        assert_eq!(pool.find("a"), Some(StringIdx(1)));
        assert_eq!(pool.find("b\0c"), Some(StringIdx(2)));
        assert_eq!(pool.find("\u{1F600}"), Some(StringIdx(4)));
        assert_eq!(pool.find("\u{FFFF}"), Some(StringIdx(5)));
        assert_eq!(pool.find("0"), Some(StringIdx(6)));
        assert_eq!(pool.find("zz"), None);

        let idx = pool.intern("zz");
        assert_eq!(idx, StringIdx(7));
        assert_eq!(pool.intern("zz"), idx);
        assert_eq!(pool.find("zz"), Some(idx));
    }

    #[test]
    fn retain_and_truncate_keep_contents() {
        let mut pool = raw_pool(&["b", "c"]);
        pool.push("a");
        assert_eq!(pool.find("a"), Some(StringIdx(2)));

        pool.retain(&HashSet::from([1, 2]));
        assert_eq!(pool.iter().collect::<Vec<_>>(), ["c", "a"]);
        assert_eq!(pool.find("a"), Some(StringIdx(1)));

        pool.truncate(1);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.find("a"), None);
    }

    #[test]
    fn write_item_copies_raw_and_encodes_owned() {
        let mut pool = raw_pool(&["a\u{1F600}"]);
        pool.push("\0z");
        let out = [pool.item(StringIdx(0)), pool.item(StringIdx(1))].concat();
        assert_eq!(out, [item("a\u{1F600}"), item("\0z")].concat());
    }

    #[test]
    fn rejects_bad_length() {
        let mut buf = item("abc");
        buf[0] = 2;
        let ids_off = buf.len() as u32;
        buf.extend_from_slice(&0u32.to_le_bytes());
        let err = StringPool::from_raw(
            DexBytes::from_vec(buf),
            ids_off,
            1,
            &ParseOptions::default(),
        );
        assert!(err.is_err());
    }
}
