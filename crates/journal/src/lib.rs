// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::hash::Hash;

use rustc_hash::FxHashSet;

/// A point a patch run can return to. A savepoint opened later compares greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Savepoint(u32);

impl Savepoint {
    pub const FIRST: Self = Self(0);

    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The number that names this savepoint to a patch runtime across a host boundary.
    pub fn id(self) -> u32 {
        self.0
    }
}

/// One owner's undo records, grouped under the savepoint that was innermost
/// when each change happened. Nothing is recorded while no savepoint is open.
///
/// A record keeps the state of a key from before its savepoint, so only the
/// first change of a key under a savepoint is recorded. A barrier marks a change
/// that moves keyed state, such as a removal that shifts later indices: keys
/// recorded before it no longer name the same state.
#[derive(Debug, Clone)]
pub struct Journal<K, U> {
    frames: Vec<Frame<K, U>>,
}

#[derive(Debug, Clone)]
struct Frame<K, U> {
    savepoint: Savepoint,
    entries: Vec<Entry<K, U>>,
    /// Keys whose state from before the savepoint is kept, since its last barrier.
    kept: FxHashSet<K>,
}

#[derive(Debug, Clone)]
enum Entry<K, U> {
    Keyed(K, U),
    Barrier(U),
}

impl<K, U> Default for Journal<K, U> {
    fn default() -> Self {
        Self { frames: Vec::new() }
    }
}

impl<K: Eq + Hash + Clone, U> Journal<K, U> {
    pub fn open(&mut self, savepoint: Savepoint) {
        assert!(
            self.innermost().is_none_or(|inner| inner < savepoint),
            "savepoints open in increasing order"
        );
        self.frames.push(Frame {
            savepoint,
            entries: Vec::new(),
            kept: FxHashSet::default(),
        });
    }

    pub fn innermost(&self) -> Option<Savepoint> {
        self.frames.last().map(|frame| frame.savepoint)
    }

    /// The open savepoints, oldest first: what an owner that joins now must open
    /// to be undone with this one.
    pub fn savepoints(&self) -> impl Iterator<Item = Savepoint> + '_ {
        self.frames.iter().map(|frame| frame.savepoint)
    }

    pub fn is_open(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Keeps `undo`, which restores `key`, unless the innermost savepoint
    /// already keeps that key. `undo` runs only when it is kept.
    pub fn record(&mut self, key: K, undo: impl FnOnce() -> U) {
        if let Some(frame) = self.frames.last_mut() {
            frame.keep(key, undo);
        }
    }

    /// Keeps `undo` for a change that moves keyed state.
    pub fn barrier(&mut self, undo: impl FnOnce() -> U) {
        if let Some(frame) = self.frames.last_mut() {
            frame.barrier(undo());
        }
    }

    /// Folds the records made under `savepoint` into the savepoint around it,
    /// which undoes them too from then on. Without one around it, they are dropped.
    pub fn release(&mut self, savepoint: Savepoint) {
        let Some(position) = self.position(savepoint) else {
            return;
        };
        let frame = self.frames.remove(position);
        let Some(outer) = position.checked_sub(1).map(|outer| &mut self.frames[outer]) else {
            return;
        };
        for entry in frame.entries {
            match entry {
                Entry::Keyed(key, undo) => outer.keep(key, || undo),
                Entry::Barrier(undo) => outer.barrier(undo),
            }
        }
    }

    /// Closes `savepoint` and every later one, returning their undo records
    /// newest first. Applying them in order restores the state at `savepoint`.
    pub fn rollback(&mut self, savepoint: Savepoint) -> Vec<U> {
        let start = self
            .frames
            .partition_point(|frame| frame.savepoint < savepoint);
        self.frames
            .drain(start..)
            .rev()
            .flat_map(|frame| frame.entries.into_iter().rev())
            .map(|entry| match entry {
                Entry::Keyed(_, undo) | Entry::Barrier(undo) => undo,
            })
            .collect()
    }

    fn position(&self, savepoint: Savepoint) -> Option<usize> {
        self.frames
            .binary_search_by_key(&savepoint, |frame| frame.savepoint)
            .ok()
    }
}

impl<K: Eq + Hash + Clone, U> Frame<K, U> {
    fn keep(&mut self, key: K, undo: impl FnOnce() -> U) {
        if self.kept.insert(key.clone()) {
            self.entries.push(Entry::Keyed(key, undo()));
        }
    }

    fn barrier(&mut self, undo: U) {
        self.kept.clear();
        self.entries.push(Entry::Barrier(undo));
    }
}

#[cfg(test)]
mod tests;
