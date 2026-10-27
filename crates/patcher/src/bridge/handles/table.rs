// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::MethodKind;
use reseam_journal::Savepoint;

use super::INVALID_HANDLE;
use super::slots::{Position, Slots};
use crate::bridge::run_state::record_failure;
use crate::context::{ClassLocation, MethodLocation};

// A method key is `dex << 56 | class << 24 | virtual << 16 | method` and a class
// key `dex << 56 | class`, so keys order like the locations they encode.
const METHOD_REMOVED: u64 = 1 << 17;
const CLASS_REMOVED: u64 = 1 << 32;
const METHOD_IN_LIST: Position = Position {
    shift: 0,
    width: 16,
};
const CLASS_IN_DEX: Position = Position {
    shift: 0,
    width: 32,
};
const CLASS_OF_METHOD: Position = Position {
    shift: 24,
    width: 32,
};

/// Stable handles for the classes and methods patches hold while the DEX they
/// name changes underneath them.
pub(crate) struct HandleTable {
    methods: Slots,
    classes: Slots,
}

impl Default for HandleTable {
    fn default() -> Self {
        Self {
            methods: Slots::new(METHOD_REMOVED),
            classes: Slots::new(CLASS_REMOVED),
        }
    }
}

impl HandleTable {
    pub fn alloc_method(&mut self, m: MethodLocation) -> u32 {
        method_key(m).map_or(INVALID_HANDLE, |key| self.methods.alloc(key))
    }

    pub fn forget_method(&mut self, m: MethodLocation) {
        if let Some(key) = method_key(m) {
            self.methods.remove(key, METHOD_IN_LIST);
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
        self.methods.relocate(handle, key);
    }

    pub fn get_method(&self, handle: u32) -> Option<MethodLocation> {
        let key = self.methods.get(handle)?;
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
        class_key(c).map_or(INVALID_HANDLE, |key| self.classes.alloc(key))
    }

    pub fn forget_class(&mut self, class: ClassLocation) {
        let Some(key) = class_key(class) else {
            return;
        };
        self.classes.remove(key, CLASS_IN_DEX);
        let methods = (class.dex_idx as u64) << 56 | (class.class_idx as u64) << 24;
        self.methods.remove(methods, CLASS_OF_METHOD);
        // Class removal can put a removed method's slot after a live method
        // in the next class. The old prefix no longer has a sortable key order.
        self.methods.unsort();
    }

    pub fn get_class(&self, handle: u32) -> Option<ClassLocation> {
        let key = self.classes.get(handle)?;
        Some(ClassLocation {
            dex_idx: (key >> 56) as usize,
            class_idx: key as u32 as usize,
        })
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
        self.methods.rollback_to(savepoint);
        self.classes.rollback_to(savepoint);
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

fn class_key(c: ClassLocation) -> Option<u64> {
    if c.dex_idx >= 256 || u32::try_from(c.class_idx).is_err() {
        record_failure(format!("class location exceeds bridge limits: {c:?}"));
        return None;
    }
    Some((c.dex_idx as u64) << 56 | c.class_idx as u64)
}
