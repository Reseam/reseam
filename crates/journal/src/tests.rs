// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Journal, Savepoint};

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Len,
    Value(usize),
}

enum Undo {
    Truncate(usize),
    Set(usize, u32),
    Insert(usize, u32),
}

/// An owner whose pushes, writes and removals are all undoable.
#[derive(Default)]
struct Values {
    values: Vec<u32>,
    journal: Journal<Key, Undo>,
}

impl Values {
    fn push(&mut self, value: u32) {
        let len = self.values.len();
        self.journal.record(Key::Len, || Undo::Truncate(len));
        self.values.push(value);
    }

    fn set(&mut self, index: usize, value: u32) {
        let old = self.values[index];
        self.journal
            .record(Key::Value(index), || Undo::Set(index, old));
        self.values[index] = value;
    }

    fn remove(&mut self, index: usize) {
        let old = self.values.remove(index);
        self.journal.barrier(|| Undo::Insert(index, old));
    }

    fn rollback(&mut self, savepoint: Savepoint) {
        for undo in self.journal.rollback(savepoint) {
            match undo {
                Undo::Truncate(len) => self.values.truncate(len),
                Undo::Set(index, value) => self.values[index] = value,
                Undo::Insert(index, value) => self.values.insert(index, value),
            }
        }
    }
}

/// A deterministic sequence, so a failure reproduces.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % bound as u64) as usize
    }
}

#[test]
fn rollback_restores_the_state_at_any_open_savepoint() {
    for seed in 0..200 {
        let mut random = Lcg(seed);
        let mut owner = Values::default();
        let mut next = Savepoint::FIRST;
        let mut open: Vec<(Savepoint, Vec<u32>)> = Vec::new();
        for _ in 0..400 {
            match random.below(10) {
                0 | 1 => {
                    open.push((next, owner.values.clone()));
                    owner.journal.open(next);
                    next = next.next();
                }
                2 if !open.is_empty() => {
                    let (savepoint, _) = open.remove(random.below(open.len()));
                    owner.journal.release(savepoint);
                }
                3 if !open.is_empty() => {
                    let position = random.below(open.len());
                    let (savepoint, expected) = open[position].clone();
                    open.truncate(position);
                    owner.rollback(savepoint);
                    assert_eq!(owner.values, expected, "seed {seed}");
                }
                4 if !owner.values.is_empty() => owner.remove(random.below(owner.values.len())),
                5..=7 if !owner.values.is_empty() => {
                    let index = random.below(owner.values.len());
                    owner.set(index, random.below(1000) as u32);
                }
                _ => owner.push(random.below(1000) as u32),
            }
        }
    }
}

#[test]
fn a_key_changed_repeatedly_under_one_savepoint_keeps_one_record() {
    let mut owner = Values::default();
    owner.push(0);
    owner.journal.open(Savepoint::FIRST);
    for value in 1..100 {
        owner.set(0, value);
    }
    let inner = Savepoint::FIRST.next();
    owner.journal.open(inner);
    owner.set(0, 500);
    owner.journal.release(inner);
    assert_eq!(owner.journal.rollback(Savepoint::FIRST).len(), 1);
}
