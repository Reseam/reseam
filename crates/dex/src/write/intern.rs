// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::sink::DexSink;
use crate::error::Result;

/// Deduplicates byte items into one flat buffer. Items are found through a
/// hash of their bytes and verified against the buffer, so each unique item
/// is stored exactly once rather than once in the buffer and once as a map key.
#[derive(Default)]
pub(crate) struct ByteInterner {
    data: Vec<u8>,
    ranges: Vec<(u32, u32)>,
    index: FxHashMap<u64, SmallVec<[u32; 1]>>,
}

impl ByteInterner {
    /// Returns the item's index, appending it when it is new.
    pub(crate) fn intern(&mut self, bytes: &[u8]) -> usize {
        let mut hasher = FxHasher::default();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        let found = self.index.get(&hash).and_then(|bucket| {
            bucket
                .iter()
                .copied()
                .find(|&i| self.get_range(self.ranges[i as usize]) == bytes)
        });
        if let Some(i) = found {
            return i as usize;
        }
        let i = self.ranges.len();
        self.ranges
            .push((self.data.len() as u32, bytes.len() as u32));
        self.data.extend_from_slice(bytes);
        self.index.entry(hash).or_default().push(i as u32);
        i
    }

    pub(crate) fn len(&self) -> usize {
        self.ranges.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Byte offset of item `i` within the flat buffer.
    pub(crate) fn offset(&self, i: usize) -> u32 {
        self.ranges[i].0
    }

    pub(crate) fn get(&self, i: usize) -> &[u8] {
        self.get_range(self.ranges[i])
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.data
    }

    fn get_range(&self, (start, len): (u32, u32)) -> &[u8] {
        &self.data[start as usize..(start + len) as usize]
    }
}

/// Deduplicates items written straight to the sink: only `(offset, len)`
/// per unique item stays in memory, and a hash hit is verified by reading
/// the earlier item back from the sink.
#[derive(Default)]
pub(crate) struct StreamInterner {
    index: FxHashMap<u64, SmallVec<[(u32, u32); 1]>>,
    count: usize,
    scratch: Vec<u8>,
}

impl StreamInterner {
    /// Writes `bytes` unless an identical item was written before, and
    /// returns the item's offset in the sink.
    pub(crate) fn intern<S: DexSink>(&mut self, sink: &mut S, bytes: &[u8]) -> Result<u32> {
        let mut hasher = FxHasher::default();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(bucket) = self.index.get(&hash) {
            for &(offset, len) in bucket {
                if len as usize != bytes.len() {
                    continue;
                }
                sink.read_back(offset as usize, len as usize, &mut self.scratch)?;
                if self.scratch == bytes {
                    return Ok(offset);
                }
            }
        }
        let offset = sink.pos();
        sink.write(bytes);
        self.index
            .entry(hash)
            .or_default()
            .push((offset, bytes.len() as u32));
        self.count += 1;
        Ok(offset)
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteInterner, StreamInterner};

    #[test]
    fn streams_unique_items_only() {
        let mut sink = Vec::new();
        let mut interner = StreamInterner::default();
        assert_eq!(interner.intern(&mut sink, b"abc").unwrap(), 0);
        assert_eq!(interner.intern(&mut sink, b"de").unwrap(), 3);
        assert_eq!(interner.intern(&mut sink, b"abc").unwrap(), 0);
        assert_eq!(interner.len(), 2);
        assert_eq!(sink, b"abcde");
    }

    #[test]
    fn interns_once_and_keeps_offsets() {
        let mut interner = ByteInterner::default();
        assert_eq!(interner.intern(b"abc"), 0);
        assert_eq!(interner.intern(b"de"), 1);
        assert_eq!(interner.intern(b"abc"), 0);
        assert_eq!(interner.len(), 2);
        assert_eq!(interner.offset(1), 3);
        assert_eq!(interner.get(1), b"de");
        assert_eq!(interner.data(), b"abcde");
    }
}
