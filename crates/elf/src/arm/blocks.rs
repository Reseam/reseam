// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::tracking::Tracker;

/// A function's blocks waiting to be walked, each with what the registers held where control
/// came from first. They are walked from the lowest address, so a function's literal loads mark
/// its pools before any walk falls through into them after a call that never returns. Functions
/// have few blocks, which makes a scan for the lowest cheaper than a tree.
pub(super) struct Blocks(Vec<(u64, Tracker)>);

impl Blocks {
    /// The blocks of a function, starting with its entry, with no register known.
    pub(super) fn new(entry: u64) -> Self {
        Self(vec![(entry, Tracker::default())])
    }

    pub(super) fn contains(&self, start: u64) -> bool {
        self.0.iter().any(|&(pending, _)| pending == start)
    }

    pub(super) fn insert(&mut self, start: u64, tracker: &Tracker) {
        if !self.contains(start) {
            self.0.push((start, tracker.clone()));
        }
    }

    pub(super) fn pop_lowest(&mut self) -> Option<(u64, Tracker)> {
        let lowest = (0..self.0.len()).min_by_key(|&index| self.0[index].0)?;
        Some(self.0.swap_remove(lowest))
    }
}
