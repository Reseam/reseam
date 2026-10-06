// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::handles::record_failure;
use crate::context::{ClassLocation, MethodLocation};
use crate::error::{PatcherError, Result as PatcherResult};
use reseam_apk::reseam_dex::MethodKind;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU32, Ordering};

const CLASS_REMOVED: u64 = 1 << 32;
const INVALID_HANDLE: u32 = u32::MAX;

const HANDLE_BLOCK: usize = 4096;
static NEXT_HANDLE_BLOCK: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
pub(crate) struct HandleSpace {
    ranges: Vec<u32>,
}

impl HandleSpace {
    pub fn allocate(&mut self, slot: usize) -> PatcherResult<u32> {
        if slot.is_multiple_of(HANDLE_BLOCK) {
            let first = NEXT_HANDLE_BLOCK
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                    next.checked_add(HANDLE_BLOCK as u32)
                })
                .map_err(|_| {
                    PatcherError::Bridge("native handle identities are exhausted".into())
                })?;
            self.ranges.push(first);
        }
        Ok(self.handle(slot))
    }

    pub fn handle(&self, slot: usize) -> u32 {
        self.ranges[slot / HANDLE_BLOCK] + (slot % HANDLE_BLOCK) as u32
    }

    pub fn slot(&self, handle: u32) -> Option<usize> {
        let range = self
            .ranges
            .partition_point(|first| *first <= handle)
            .checked_sub(1)?;
        let offset = handle - self.ranges[range];
        (offset < HANDLE_BLOCK as u32).then_some(range * HANDLE_BLOCK + offset as usize)
    }
}

#[derive(Default)]
pub(crate) struct HandleTable {
    methods: Handles,
    classes: Handles,
}

#[derive(Default)]
struct Handles {
    identities: HandleSpace,
    keys: Vec<u64>,
    sorted_len: usize,
    index: FxHashMap<u64, u32>,
    relocated: FxHashMap<u32, u64>,
}

const METHOD_REMOVED: u64 = 1 << 17;

impl Handles {
    fn alloc(&mut self, key: u64, removed_bit: u64) -> u32 {
        if let Some(&h) = self.index.get(&key) {
            return h;
        }
        let sorted = &self.keys[..self.sorted_len];
        let first = sorted.partition_point(|held| held & !removed_bit < key);
        if let Some(offset) = sorted[first..]
            .iter()
            .take_while(|held| **held & !removed_bit == key)
            .position(|held| held & removed_bit == 0)
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
        if self.sorted_len == self.keys.len()
            && self
                .keys
                .last()
                .is_none_or(|&last| last & !removed_bit < key)
        {
            self.sorted_len += 1;
        } else {
            self.index.insert(key, h);
        }
        self.keys.push(key);
        h
    }

    fn reindex(&mut self, removed_bit: u64) {
        self.index = self
            .keys
            .iter()
            .enumerate()
            .skip(self.sorted_len)
            .filter(|(_, key)| **key & removed_bit == 0)
            .map(|(slot, key)| (*key, self.identities.handle(slot)))
            .chain(
                self.relocated
                    .iter()
                    .filter(|(_, key)| **key & removed_bit == 0)
                    .map(|(handle, key)| (*key, *handle)),
            )
            .collect();
    }

    fn get(&self, handle: u32) -> Option<u64> {
        self.relocated
            .get(&handle)
            .copied()
            .or_else(|| self.keys.get(self.identities.slot(handle)?).copied())
    }
}

impl HandleTable {
    pub fn alloc_method(&mut self, m: MethodLocation) -> u32 {
        method_key(m).map_or(INVALID_HANDLE, |key| {
            self.methods.alloc(key, METHOD_REMOVED)
        })
    }

    pub fn forget_method(&mut self, m: MethodLocation) {
        let Some(key) = method_key(m) else {
            return;
        };
        let group = key & !(0xffff | METHOD_REMOVED);
        let slot = key as u16;
        let affected: Vec<_> = self
            .methods
            .keys
            .iter()
            .enumerate()
            .filter(|(_, held)| {
                **held & !(0xffff | METHOD_REMOVED) == group
                    && ((**held as u16) > slot
                        || (**held & METHOD_REMOVED == 0 && (**held as u16) == slot))
            })
            .map(|(handle, _)| handle)
            .collect();
        for &handle in &affected {
            if handle >= self.methods.sorted_len && self.methods.keys[handle] & METHOD_REMOVED == 0
            {
                self.methods.index.remove(&self.methods.keys[handle]);
            }
        }
        for handle in affected {
            let held = &mut self.methods.keys[handle];
            if *held as u16 == slot {
                *held |= METHOD_REMOVED;
            } else {
                *held -= 1;
                if handle >= self.methods.sorted_len && *held & METHOD_REMOVED == 0 {
                    self.methods
                        .index
                        .insert(*held, self.methods.identities.handle(handle));
                }
            }
        }
        for (&handle, held) in &mut self.methods.relocated {
            if *held & METHOD_REMOVED != 0
                || *held & !(0xffff | METHOD_REMOVED) != group
                || (*held as u16) < slot
            {
                continue;
            }
            self.methods.index.remove(held);
            if *held as u16 == slot {
                *held |= METHOD_REMOVED;
            } else {
                *held -= 1;
                self.methods.index.insert(*held, handle);
            }
        }
    }

    pub fn relocate_method(&mut self, old: MethodLocation, new: MethodLocation) {
        let Some(key) = method_key(new) else {
            return;
        };
        let handle = self.alloc_method(old);
        if handle == INVALID_HANDLE {
            return;
        }
        self.forget_method(old);
        self.methods.keys[self
            .methods
            .identities
            .slot(handle)
            .expect("allocated handle names its table slot")] |= METHOD_REMOVED;
        self.methods.relocated.insert(handle, key);
        self.methods.index.insert(key, handle);
    }

    pub fn get_method(&self, handle: u32) -> Option<MethodLocation> {
        let key = self
            .methods
            .get(handle)
            .filter(|key| key & METHOD_REMOVED == 0)?;
        Some(MethodLocation {
            dex_idx: (key >> 56) as usize,
            class_idx: (key >> 24) as u32 as usize,
            method_idx: key as u16 as usize,
            kind: if (key >> 16) & 1 == 1 {
                MethodKind::Virtual
            } else {
                MethodKind::Direct
            },
        })
    }

    pub fn alloc_class(&mut self, c: ClassLocation) -> u32 {
        if c.dex_idx >= 256 || u32::try_from(c.class_idx).is_err() {
            record_failure(format!("class location exceeds bridge limits: {c:?}"));
            return INVALID_HANDLE;
        }
        self.classes
            .alloc((c.dex_idx as u64) << 56 | c.class_idx as u64, CLASS_REMOVED)
    }

    pub fn forget_class(&mut self, class: ClassLocation) {
        for key in &mut self.classes.keys {
            if (*key >> 56) as usize != class.dex_idx {
                continue;
            }
            match (*key as u32 as usize).cmp(&class.class_idx) {
                std::cmp::Ordering::Equal => *key |= CLASS_REMOVED,
                std::cmp::Ordering::Greater => *key -= 1,
                std::cmp::Ordering::Less => {}
            }
        }
        for key in &mut self.methods.keys {
            if (*key >> 56) as usize != class.dex_idx {
                continue;
            }
            match ((*key >> 24) as u32 as usize).cmp(&class.class_idx) {
                std::cmp::Ordering::Equal => *key |= METHOD_REMOVED,
                std::cmp::Ordering::Greater => *key -= 1 << 24,
                std::cmp::Ordering::Less => {}
            }
        }
        for key in self.methods.relocated.values_mut() {
            if (*key >> 56) as usize != class.dex_idx {
                continue;
            }
            match ((*key >> 24) as u32 as usize).cmp(&class.class_idx) {
                std::cmp::Ordering::Equal => *key |= METHOD_REMOVED,
                std::cmp::Ordering::Greater => *key -= 1 << 24,
                std::cmp::Ordering::Less => {}
            }
        }
        self.classes.reindex(CLASS_REMOVED);
        // Class removal can put a removed method's slot after a live method
        // in the next class. The old prefix no longer has a sortable key order.
        self.methods.sorted_len = 0;
        self.methods.reindex(METHOD_REMOVED);
    }

    pub fn get_class(&self, handle: u32) -> Option<ClassLocation> {
        let key = self
            .classes
            .get(handle)
            .filter(|key| key & CLASS_REMOVED == 0)?;
        Some(ClassLocation {
            dex_idx: (key >> 56) as usize,
            class_idx: key as u32 as usize,
        })
    }
}

fn method_key(m: MethodLocation) -> Option<u64> {
    if m.dex_idx >= 256
        || u32::try_from(m.class_idx).is_err()
        || u16::try_from(m.method_idx).is_err()
    {
        record_failure(format!("method location exceeds bridge limits: {m:?}"));
        return None;
    }
    Some(
        (m.dex_idx as u64) << 56
            | (m.class_idx as u64) << 24
            | u64::from(m.kind == MethodKind::Virtual) << 16
            | m.method_idx as u64,
    )
}
