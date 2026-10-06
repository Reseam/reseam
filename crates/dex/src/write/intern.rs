// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::sink::DexSink;
use crate::error::Result;

const PREFIX_LIMIT: usize = 128 << 10;

pub(crate) struct ByteInterner {
    data: super::sink::SpoolSink,
    ranges: Vec<(u32, u32)>,
    index: FxHashMap<u64, SmallVec<[u32; 1]>>,
    scratch: Vec<u8>,
    prefix: Vec<u8>,
}

impl ByteInterner {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            data: super::sink::SpoolSink::new()?,
            ranges: Vec::new(),
            index: FxHashMap::default(),
            scratch: Vec::new(),
            prefix: Vec::new(),
        })
    }

    pub(crate) fn intern(&mut self, bytes: &[u8]) -> Result<usize> {
        let mut hasher = FxHasher::default();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(bucket) = self.index.get(&hash) {
            for &index in bucket {
                let (offset, len) = self.ranges[index as usize];
                if len as usize != bytes.len() {
                    continue;
                }
                let range = offset as usize..offset as usize + len as usize;
                let previous = if let Some(previous) = self.prefix.get(range) {
                    previous
                } else {
                    self.data
                        .read_back(offset as usize, len as usize, &mut self.scratch)?;
                    &self.scratch
                };
                if previous == bytes {
                    return Ok(index as usize);
                }
            }
        }
        let index = self.ranges.len();
        self.ranges.push((self.data.pos(), bytes.len() as u32));
        let retained = bytes.len().min(PREFIX_LIMIT - self.prefix.len());
        self.prefix.extend_from_slice(&bytes[..retained]);
        self.data.write(bytes);
        self.index.entry(hash).or_default().push(index as u32);
        Ok(index)
    }

    pub(crate) fn len(&self) -> usize {
        self.ranges.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
    pub(crate) fn offset(&self, index: usize) -> u32 {
        self.ranges[index].0
    }

    pub(crate) fn get(&self, index: usize, bytes: &mut Vec<u8>) -> Result<()> {
        let (offset, len) = self.ranges[index];
        self.data.read_back(offset as usize, len as usize, bytes)
    }

    pub(crate) fn write_to<S: DexSink>(&mut self, sink: &mut S) -> Result<()> {
        self.data
            .digest(0, self.data.pos() as usize, &mut |bytes| sink.write(bytes))
    }
}

#[derive(Default)]
pub(crate) struct StreamInterner {
    index: FxHashMap<u64, SmallVec<[(u32, u32); 1]>>,
    count: usize,
    scratch: Vec<u8>,
}

impl StreamInterner {
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
