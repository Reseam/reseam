// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::{Journal, Savepoint};
use rustc_hash::FxHashMap;

use super::INVALID_HANDLE;
use super::space::HandleSpace;
use crate::bridge::run_state::record_failure;

/// Handles over ordered `u64` location keys. A key with `removed` set names a
/// location that no longer exists; its handle stays dead. Keys allocated in
/// ascending order form a sorted prefix searched by bisection, the rest and
/// relocated handles are found through `index`.
pub(super) struct Slots {
    removed: u64,
    identities: HandleSpace,
    keys: Vec<u64>,
    sorted_len: usize,
    index: FxHashMap<u64, u32>,
    relocated: FxHashMap<u32, u64>,
    journal: Journal<Part, Undo>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Part {
    Len,
    SortedLen,
    Key(usize),
    Relocated(u32),
}

enum Undo {
    /// Slots from this length on were allocated since; they end removed, so
    /// their handles are never given to another location.
    Len(usize),
    SortedLen(usize),
    Key(usize, u64),
    Relocated(u32, Option<u64>),
}

impl Slots {
    pub(super) fn new(removed: u64) -> Self {
        Self {
            removed,
            identities: HandleSpace::default(),
            keys: Vec::new(),
            sorted_len: 0,
            index: FxHashMap::default(),
            relocated: FxHashMap::default(),
            journal: Journal::default(),
        }
    }

    fn is_live(&self, key: u64) -> bool {
        key & self.removed == 0
    }

    pub(super) fn alloc(&mut self, key: u64) -> u32 {
        if let Some(&h) = self.index.get(&key) {
            return h;
        }
        let removed = self.removed;
        let sorted = &self.keys[..self.sorted_len];
        let first = sorted.partition_point(|held| held & !removed < key);
        if let Some(offset) = sorted[first..]
            .iter()
            .take_while(|held| **held & !removed == key)
            .position(|held| held & removed == 0)
        {
            return self.identities.handle(first + offset);
        }
        let h = match self.identities.allocate(self.keys.len()) {
            Ok(handle) => handle,
            Err(error) => {
                record_failure(error);
                return INVALID_HANDLE;
            }
        };
        let len = self.keys.len();
        self.journal.record(Part::Len, || Undo::Len(len));
        if self.sorted_len == self.keys.len()
            && self.keys.last().is_none_or(|&last| last & !removed < key)
        {
            self.sorted_len += 1;
        } else {
            self.index.insert(key, h);
        }
        self.keys.push(key);
        h
    }

    /// The live key `handle` names.
    pub(super) fn get(&self, handle: u32) -> Option<u64> {
        self.relocated
            .get(&handle)
            .copied()
            .or_else(|| self.keys.get(self.identities.slot(handle)?).copied())
            .filter(|&key| self.is_live(key))
    }

    /// Removes the list entry `entry` names at `position`: its keys end
    /// removed, and keys of later entries in the same list move down one.
    pub(super) fn remove(&mut self, entry: u64, position: Position) {
        let removed = self.removed;
        let list = |key: u64| (key & !removed) >> (position.shift + position.width);
        let at = |key: u64| (key >> position.shift) & ((1 << position.width) - 1);
        let changes = self
            .keys
            .iter()
            .enumerate()
            .map(|(slot, &key)| (Held::Slot(slot), key))
            .chain(
                self.relocated
                    .iter()
                    .map(|(&handle, &key)| (Held::Relocated(handle), key)),
            )
            .filter(|&(_, key)| list(key) == list(entry))
            .filter_map(|(held, key)| {
                let moved = match at(key).cmp(&at(entry)) {
                    std::cmp::Ordering::Equal => key | removed,
                    std::cmp::Ordering::Greater => key - (1 << position.shift),
                    std::cmp::Ordering::Less => key,
                };
                (moved != key).then_some((held, moved))
            })
            .collect();
        self.rekey(changes);
    }

    /// Gives each held key its new value, keeping `index` in step.
    fn rekey(&mut self, changes: Vec<(Held, u64)>) {
        for &(held, _) in &changes {
            if let Some((key, _)) = self.indexed(held) {
                self.index.remove(&key);
            }
        }
        for (held, key) in changes {
            match held {
                Held::Slot(slot) => {
                    let previous = std::mem::replace(&mut self.keys[slot], key);
                    self.journal
                        .record(Part::Key(slot), || Undo::Key(slot, previous));
                }
                Held::Relocated(handle) => self.set_relocated(handle, key),
            }
            if let Some((key, handle)) = self.indexed(held) {
                self.index.insert(key, handle);
            }
        }
    }

    /// The `index` entry `held` has: a live key outside the sorted prefix.
    fn indexed(&self, held: Held) -> Option<(u64, u32)> {
        let (key, handle) = match held {
            Held::Slot(slot) if slot >= self.sorted_len => {
                (self.keys[slot], self.identities.handle(slot))
            }
            Held::Slot(_) => return None,
            Held::Relocated(handle) => (*self.relocated.get(&handle)?, handle),
        };
        self.is_live(key).then_some((key, handle))
    }

    /// Moves `handle` to `key`; its slot ends removed.
    pub(super) fn relocate(&mut self, handle: u32, key: u64) {
        let slot = self
            .identities
            .slot(handle)
            .expect("allocated handle names its table slot");
        self.rekey(vec![
            (Held::Slot(slot), self.keys[slot] | self.removed),
            (Held::Relocated(handle), key),
        ]);
    }

    /// Drops the sorted prefix after keys changed order.
    pub(super) fn unsort(&mut self) {
        let previous = std::mem::replace(&mut self.sorted_len, 0);
        self.journal
            .record(Part::SortedLen, || Undo::SortedLen(previous));
        self.reindex();
    }

    fn set_relocated(&mut self, handle: u32, key: u64) {
        let previous = self.relocated.insert(handle, key);
        self.journal.record(Part::Relocated(handle), || {
            Undo::Relocated(handle, previous)
        });
    }

    #[cfg(feature = "kotlin")]
    pub(super) fn savepoints(&self) -> impl Iterator<Item = Savepoint> + '_ {
        self.journal.savepoints()
    }

    pub(super) fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
    }

    pub(super) fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
    }

    pub(super) fn rollback_to(&mut self, savepoint: Savepoint) {
        let undos = self.journal.rollback(savepoint);
        if undos.is_empty() {
            return;
        }
        for undo in undos {
            match undo {
                Undo::Len(len) => {
                    let removed = self.removed;
                    self.keys[len..].iter_mut().for_each(|key| *key |= removed);
                }
                Undo::SortedLen(sorted_len) => self.sorted_len = sorted_len,
                Undo::Key(slot, key) => self.keys[slot] = key,
                Undo::Relocated(handle, Some(key)) => {
                    self.relocated.insert(handle, key);
                }
                Undo::Relocated(handle, None) => {
                    self.relocated.remove(&handle);
                }
            }
        }
        self.reindex();
    }

    fn reindex(&mut self) {
        self.index = self
            .keys
            .iter()
            .enumerate()
            .skip(self.sorted_len)
            .map(|(slot, &key)| (key, self.identities.handle(slot)))
            .chain(self.relocated.iter().map(|(&handle, &key)| (key, handle)))
            .filter(|&(key, _)| self.is_live(key))
            .collect();
    }
}

/// Where an entry's position sits in a key: `width` bits from `shift`. The
/// bits above it, the removed bit aside, name the list the entry is in.
#[derive(Clone, Copy)]
pub(super) struct Position {
    pub(super) shift: u32,
    pub(super) width: u32,
}

/// Where a key is held: a table slot, or a handle relocated off its slot.
#[derive(Clone, Copy)]
enum Held {
    Slot(usize),
    Relocated(u32),
}
