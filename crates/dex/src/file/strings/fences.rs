// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;
use std::ops::Range;

/// Sorted entries between two fences.
const STEP: usize = 64;

/// Every [`STEP`]th string of a pool's sorted prefix, copied out of the file.
/// A lookup first compares against these, so it reads the file only within
/// the one run of entries that can hold the string, where a binary search over
/// the whole pool reads a page of ids and a page of string data at every step.
#[derive(Debug, Clone, Default)]
pub(super) struct Fences {
    /// The fences' payloads, each through its terminator, end to end.
    bytes: Vec<u8>,
    /// Where each fence ends in `bytes`.
    ends: Vec<u32>,
}

impl Fences {
    /// Fences over `sorted` entries, where `item(i)` is entry `i`'s payload
    /// through its terminator.
    pub(super) fn of<'a>(sorted: usize, item: impl Fn(usize) -> &'a [u8]) -> Self {
        let mut fences = Self::default();
        for index in (0..sorted).step_by(STEP) {
            fences.bytes.extend_from_slice(item(index));
            fences.ends.push(fences.bytes.len() as u32);
        }
        fences.bytes.shrink_to_fit();
        fences
    }

    /// The sorted entries among the first `sorted` that can equal the string
    /// sought, given how a payload orders against it.
    pub(super) fn window(&self, sorted: usize, order: impl Fn(&[u8]) -> Ordering) -> Range<usize> {
        let (mut lo, mut hi) = (0, self.ends.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = mid.checked_sub(1).map_or(0, |before| self.ends[before]);
            if order(&self.bytes[start as usize..self.ends[mid] as usize]) == Ordering::Greater {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        match lo.checked_sub(1) {
            Some(fence) => fence * STEP..(lo * STEP).min(sorted),
            None => 0..0,
        }
    }

    pub(super) fn heap_bytes(&self) -> usize {
        self.bytes.len() + self.ends.len() * size_of::<u32>()
    }
}
