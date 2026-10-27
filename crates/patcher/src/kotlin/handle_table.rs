// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::handles::record_failure;
use crate::context::{ClassLocation, MethodLocation};
use crate::error::{PatcherError, Result as PatcherResult};
use reseam_dex::MethodKind;
use reseam_journal::Savepoint;
use std::sync::atomic::{AtomicU32, Ordering};

mod slots;

use slots::Handles;

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

const METHOD_REMOVED: u64 = 1 << 17;

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
            let held = self.methods.keys[handle];
            if held as u16 == slot {
                self.methods.set_key(handle, held | METHOD_REMOVED);
            } else {
                self.methods.set_key(handle, held - 1);
                if handle >= self.methods.sorted_len && held & METHOD_REMOVED == 0 {
                    self.methods
                        .index
                        .insert(held - 1, self.methods.identities.handle(handle));
                }
            }
        }
        let relocated: Vec<_> = self
            .methods
            .relocated
            .iter()
            .filter(|&(_, &held)| {
                held & METHOD_REMOVED == 0
                    && held & !(0xffff | METHOD_REMOVED) == group
                    && held as u16 >= slot
            })
            .map(|(&handle, &held)| (handle, held))
            .collect();
        for (handle, held) in relocated {
            self.methods.index.remove(&held);
            if held as u16 == slot {
                self.methods.set_relocated(handle, held | METHOD_REMOVED);
            } else {
                self.methods.set_relocated(handle, held - 1);
                self.methods.index.insert(held - 1, handle);
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
        let slot = self
            .methods
            .identities
            .slot(handle)
            .expect("allocated handle names its table slot");
        self.methods
            .set_key(slot, self.methods.keys[slot] | METHOD_REMOVED);
        self.methods.set_relocated(handle, key);
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
        let classes: Vec<_> = self
            .classes
            .keys
            .iter()
            .enumerate()
            .filter_map(|(slot, &key)| Some((slot, after_removal(key, class, 0, CLASS_REMOVED)?)))
            .collect();
        for (slot, key) in classes {
            self.classes.set_key(slot, key);
        }
        let methods: Vec<_> = self
            .methods
            .keys
            .iter()
            .enumerate()
            .filter_map(|(slot, &key)| Some((slot, after_removal(key, class, 24, METHOD_REMOVED)?)))
            .collect();
        for (slot, key) in methods {
            self.methods.set_key(slot, key);
        }
        let relocated: Vec<_> = self
            .methods
            .relocated
            .iter()
            .filter_map(|(&handle, &key)| {
                Some((handle, after_removal(key, class, 24, METHOD_REMOVED)?))
            })
            .collect();
        for (handle, key) in relocated {
            self.methods.set_relocated(handle, key);
        }
        self.classes.reindex(CLASS_REMOVED);
        // Class removal can put a removed method's slot after a live method
        // in the next class. The old prefix no longer has a sortable key order.
        self.methods.set_sorted_len(0);
        self.methods.reindex(METHOD_REMOVED);
    }

    /// The open savepoints, oldest first.
    #[cfg(feature = "kotlin")]
    pub fn savepoints(&self) -> Vec<Savepoint> {
        self.methods.savepoints().collect()
    }

    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.methods.open_savepoint(savepoint);
        self.classes.open_savepoint(savepoint);
    }

    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.methods.release_savepoint(savepoint);
        self.classes.release_savepoint(savepoint);
    }

    /// Handles allocated since `savepoint` end removed rather than reused, so
    /// a handle never names two locations in one run.
    pub fn rollback_to(&mut self, savepoint: Savepoint) {
        self.methods.rollback_to(savepoint, METHOD_REMOVED);
        self.classes.rollback_to(savepoint, CLASS_REMOVED);
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

/// `key`, a class or method key whose class position starts at bit `shift`,
/// after class `class` is removed: removed when it is in that class, a class
/// lower when it is after it, and unchanged (`None`) otherwise.
fn after_removal(key: u64, class: ClassLocation, shift: u32, removed_bit: u64) -> Option<u64> {
    if (key >> 56) as usize != class.dex_idx {
        return None;
    }
    match ((key >> shift) as u32 as usize).cmp(&class.class_idx) {
        std::cmp::Ordering::Equal => Some(key | removed_bit),
        std::cmp::Ordering::Greater => Some(key - (1 << shift)),
        std::cmp::Ordering::Less => None,
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
