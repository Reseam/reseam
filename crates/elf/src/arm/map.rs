// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use fixedbitset::{Block, FixedBitSet};
use reseam_bytes::u16_at;

use super::mode::Mode;
use super::t32;
use crate::Function;
use crate::error::Result;
use crate::file::{ElfImage, Section};

const BLOCK_BITS: usize = Block::BITS as usize;

/// What the walk found in one code section, by halfword: where functions and instructions start,
/// and what code or data covers.
pub(super) struct SectionCode {
    /// The section's index in the library.
    index: usize,
    pub(super) address: u64,
    size: u64,
    starts: FixedBitSet,
    instructions: FixedBitSet,
    /// Of the instructions, the Thumb ones.
    thumb: FixedBitSet,
    /// Halfwords an instruction or the data one reads covers.
    taken: FixedBitSet,
}

impl SectionCode {
    pub(super) fn new(index: usize, section: &Section) -> Self {
        let halfwords = section.size.div_ceil(2) as usize;
        Self {
            index,
            address: section.address,
            size: section.size,
            starts: FixedBitSet::with_capacity(halfwords),
            instructions: FixedBitSet::with_capacity(halfwords),
            thumb: FixedBitSet::with_capacity(halfwords),
            taken: FixedBitSet::with_capacity(halfwords),
        }
    }

    /// The section's contents.
    pub(super) fn bytes<'a>(&self, image: &'a ElfImage) -> Result<&'a [u8]> {
        image.contents(&image.sections()[self.index])
    }

    pub(super) fn contains(&self, address: u64) -> bool {
        address
            .checked_sub(self.address)
            .is_some_and(|at| at < self.size)
    }

    pub(super) fn end(&self) -> u64 {
        self.address + self.size
    }

    fn halfword(&self, address: u64) -> usize {
        ((address - self.address) / 2) as usize
    }

    /// The halfwords that `size` bytes from `address` overlap.
    fn halfwords(&self, address: u64, size: u64) -> Range<usize> {
        self.halfword(address)..self.halfword(address + size - 1) + 1
    }

    fn address_of(&self, halfword: usize) -> u64 {
        self.address + halfword as u64 * 2
    }

    /// Records a function start at `address`, which the section contains.
    pub(super) fn insert_start(&mut self, address: u64) {
        let at = self.halfword(address);
        self.starts.insert(at);
    }

    pub(super) fn is_start(&self, address: u64) -> bool {
        self.contains(address) && self.starts.contains(self.halfword(address))
    }

    /// The first function start after `address`, or the section's end.
    pub(super) fn next_start(&self, address: u64) -> u64 {
        next_one(&self.starts, self.halfword(address) + 1)
            .map_or(self.end(), |at| self.address_of(at))
    }

    /// The function whose code holds `address`, which the section contains: from the last start
    /// up to it to the end of the last instruction before the next, in the instruction set of
    /// the instruction holding `address`. `code` is the section's contents.
    pub(super) fn function(&self, code: &[u8], address: u64) -> Option<Function> {
        let at = self.halfword(address);
        let start = previous_one(&self.starts, at)?;
        let holding = previous_one(&self.instructions, at).filter(|&holding| holding >= start)?;
        let next = next_one(&self.starts, start + 1).unwrap_or(self.starts.len());
        let last = previous_one(&self.instructions, next - 1).filter(|&last| last >= start)?;
        let end = self.address_of(last) + self.size(code, last);
        Some(Function {
            address: self.address_of(start),
            size: end - self.address_of(start),
            instruction_set: self.mode(holding).into(),
        })
        .filter(|function| function.contains(address))
    }

    fn mode(&self, instruction: usize) -> Mode {
        if self.thumb.contains(instruction) {
            Mode::Thumb
        } else {
            Mode::Arm
        }
    }

    /// The size of the instruction starting at halfword `instruction` of the section's contents
    /// `code`.
    fn size(&self, code: &[u8], instruction: usize) -> u64 {
        let narrow = self.thumb.contains(instruction)
            && !t32::is_wide(u32::from(u16_at(code, instruction * 2)));
        if narrow { 2 } else { 4 }
    }

    /// The instructions starting in `range`, which the section contains, with their instruction
    /// set.
    pub(super) fn instructions_in(&self, range: Range<u64>) -> impl Iterator<Item = (u64, Mode)> {
        let first = self.halfword(range.start.next_multiple_of(2));
        let end = self.halfword(range.end.min(self.end()).next_multiple_of(2));
        (first..end)
            .filter(|&at| self.instructions.contains(at))
            .map(|at| (self.address_of(at), self.mode(at)))
    }

    /// Whether `size` bytes from `address` lie in the section and nothing covers them yet.
    pub(super) fn is_free(&self, address: u64, size: u64) -> bool {
        self.contains(address)
            && address + size <= self.end()
            && !self
                .taken
                .contains_any_in_range(self.halfwords(address, size))
    }

    pub(super) fn is_instruction(&self, address: u64) -> bool {
        self.contains(address) && self.instructions.contains(self.halfword(address))
    }

    pub(super) fn take_instruction(&mut self, address: u64, size: u64, mode: Mode) {
        let at = self.halfword(address);
        self.instructions.insert(at);
        if mode == Mode::Thumb {
            self.thumb.insert(at);
        }
        self.take(address, size);
    }

    /// Covers the halfwords that `size` bytes from `address` overlap, as far as the section goes.
    pub(super) fn take(&mut self, address: u64, size: u64) {
        let size = size.min(self.end() - address);
        self.taken.insert_range(self.halfwords(address, size));
    }

    /// Covers a `tbb` or `tbh` table at `base` in the section's contents `code`, and returns its
    /// targets: offsets in halfwords from `base`. The table ends where the earliest target begins,
    /// since its cases follow it.
    pub(super) fn take_table(&mut self, code: &[u8], base: u64, halfwords: bool) -> Vec<u64> {
        let size = if halfwords { 2 } else { 1 };
        let mut end = self.end();
        let mut targets = Vec::new();
        let mut at = base;
        while at + size <= end && self.is_free(at & !1, 2) {
            let offset = (at - self.address) as usize;
            let entry = if halfwords {
                u64::from(u16_at(code, offset))
            } else {
                u64::from(code[offset])
            };
            let target = base + 2 * entry;
            if target < at + size {
                break;
            }
            end = end.min(target);
            targets.push(target);
            at += size;
        }
        self.take(base, at - base);
        targets
    }
}

/// The first set bit at or after `from`.
fn next_one(bits: &FixedBitSet, from: usize) -> Option<usize> {
    let blocks = bits.as_slice();
    let mut index = from / BLOCK_BITS;
    let mut block = *blocks.get(index)? & (Block::MAX << (from % BLOCK_BITS));
    while block == 0 {
        index += 1;
        block = *blocks.get(index)?;
    }
    Some(index * BLOCK_BITS + block.trailing_zeros() as usize).filter(|&at| at < bits.len())
}

/// The last set bit at or before `through`.
fn previous_one(bits: &FixedBitSet, through: usize) -> Option<usize> {
    let blocks = bits.as_slice();
    let mut index = through / BLOCK_BITS;
    let shift = BLOCK_BITS - 1 - through % BLOCK_BITS;
    let mut block = *blocks.get(index)? & (Block::MAX >> shift);
    while block == 0 {
        index = index.checked_sub(1)?;
        block = blocks[index];
    }
    Some(index * BLOCK_BITS + BLOCK_BITS - 1 - block.leading_zeros() as usize)
}
