// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod by_type;
mod lifecycle;
mod undo;

use std::sync::OnceLock;

use crate::DexBytes;
use crate::error::Result;
use crate::model::class::{ClassDef, ClassHeader};
use crate::model::{TypeIdx, TypeList};
use crate::read::class::{RECORD_SIZE, RawClassDef};
use crate::read::type_list::{read_type_list, validate_type_list};
use crate::search::index::ClassRefs;
use by_type::TypeIndex;
pub(crate) use undo::Undone;
use undo::{ClassKey, ClassUndo, Prior};

#[derive(Debug, Clone)]
enum ClassSlot {
    Raw(u32),
    Resident(Box<ResidentClass>),
}

/// A decoded class and, once a scan asks, the references of its methods.
#[derive(Debug, Clone)]
struct ResidentClass {
    def: ClassDef,
    refs: OnceLock<ClassRefs>,
}

impl ResidentClass {
    /// Mutable access, which makes the cached references stale.
    fn grant(&mut self) -> &mut ClassDef {
        self.refs.take();
        &mut self.def
    }
}

impl ClassSlot {
    fn resident(def: ClassDef) -> Self {
        Self::Resident(Box::new(ResidentClass {
            def,
            refs: OnceLock::new(),
        }))
    }
}

/// The class table: every class starts as a 32-byte record in the file and
/// is decoded into a [`ClassDef`] only when something mutates it. Classes
/// added after parse are resident from the start.
#[derive(Debug, Clone, Default)]
pub struct ClassTable {
    raw: Option<DexBytes>,
    off: usize,
    slots: Vec<ClassSlot>,
    /// Indices of the resident slots, ascending.
    resident: Vec<u32>,
    by_type: OnceLock<TypeIndex>,
    journal: reseam_journal::Journal<ClassKey, ClassUndo>,
}

impl ClassTable {
    pub(crate) fn from_raw(raw: DexBytes, off: u32, count: u32) -> Result<Self> {
        let buf = raw.as_bytes();
        let off = off as usize;
        let count = count as usize;
        crate::error::require_array(buf, off, count, RECORD_SIZE, "class definitions")?;
        for i in 0..count {
            let def = RawClassDef::read(buf, off + i * RECORD_SIZE);
            if def.interfaces_off != 0 {
                validate_type_list(buf, def.interfaces_off as usize)?;
            }
        }
        Ok(Self {
            raw: Some(raw),
            off,
            slots: (0..count as u32).map(ClassSlot::Raw).collect(),
            resident: Vec::new(),
            by_type: OnceLock::new(),
            journal: reseam_journal::Journal::default(),
        })
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn header(&self, i: usize) -> ClassHeader {
        match &self.slots[i] {
            ClassSlot::Raw(r) => self.record(*r).header(),
            ClassSlot::Resident(c) => ClassHeader::of(&c.def),
        }
    }

    pub fn headers(&self) -> impl ExactSizeIterator<Item = ClassHeader> + '_ {
        (0..self.len()).map(|i| self.header(i))
    }

    pub fn interfaces(&self, i: usize) -> TypeList {
        match &self.slots[i] {
            ClassSlot::Raw(r) => {
                let off = self.record(*r).interfaces_off;
                if off == 0 {
                    TypeList::new()
                } else {
                    read_type_list(self.raw_bytes(), off as usize)
                }
            }
            ClassSlot::Resident(c) => c.def.interfaces.clone(),
        }
    }

    /// The file record of a class that has not been materialized.
    pub fn raw_def(&self, i: usize) -> Option<RawClassDef> {
        match self.slots.get(i)? {
            ClassSlot::Raw(r) => Some(self.record(*r)),
            ClassSlot::Resident(_) => None,
        }
    }

    /// Where a file class's `class_data_item` starts; `None` for resident
    /// classes and classes without class data.
    pub(crate) fn class_data_offset(&self, i: usize) -> Option<u32> {
        self.raw_def(i)
            .map(|def| def.class_data_off)
            .filter(|&off| off != 0)
    }

    pub fn resident(&self, i: usize) -> Option<&ClassDef> {
        match self.slots.get(i)? {
            ClassSlot::Resident(c) => Some(&c.def),
            ClassSlot::Raw(_) => None,
        }
    }

    /// The references of a resident class's methods, kept until the class is
    /// next granted mutably.
    pub(crate) fn resident_refs(&self, i: usize) -> Option<&ClassRefs> {
        match self.slots.get(i)? {
            ClassSlot::Resident(c) => {
                let data = c.def.class_data.as_deref()?;
                Some(c.refs.get_or_init(|| ClassRefs::of(data)))
            }
            ClassSlot::Raw(_) => None,
        }
    }

    pub fn resident_mut(&mut self, i: usize) -> Option<&mut ClassDef> {
        if !self.is_resident(i) {
            return None;
        }
        if let Some(index) = self.by_type.get_mut() {
            index.changed.insert(i as u32);
        }
        match self.slots.get_mut(i)? {
            ClassSlot::Resident(c) => {
                self.journal.record(ClassKey::Slot(i as u32), || {
                    ClassUndo::Restore(i as u32, Prior::Resident(Box::new(c.def.clone())))
                });
                Some(c.grant())
            }
            ClassSlot::Raw(_) => None,
        }
    }

    pub(crate) fn release_refs(&mut self) {
        for slot in &mut self.slots {
            if let ClassSlot::Resident(c) = slot {
                c.refs.take();
            }
        }
    }

    /// Every resident class, in table order. Classes still in the file are
    /// skipped; call [`Self::materialize_all`] first to see all of them.
    pub fn iter_resident(&self) -> impl Iterator<Item = &ClassDef> {
        self.slots.iter().filter_map(|s| match s {
            ClassSlot::Resident(c) => Some(&c.def),
            ClassSlot::Raw(_) => None,
        })
    }

    pub(crate) fn resident_indices(&self) -> &[u32] {
        &self.resident
    }

    pub fn is_resident(&self, i: usize) -> bool {
        matches!(self.slots.get(i), Some(ClassSlot::Resident(_)))
    }

    pub fn heap_bytes(&self) -> u64 {
        (self.slots.len() * size_of::<ClassSlot>()
            + self.resident.len() * (size_of::<ClassDef>() + size_of::<u32>())
            + self
                .slots
                .iter()
                .filter_map(|s| match s {
                    ClassSlot::Resident(c) => c.refs.get().map(ClassRefs::heap_bytes),
                    ClassSlot::Raw(_) => None,
                })
                .sum::<usize>()
            + self.by_type.get().map_or(0, |index| {
                index.ordered.len() * size_of::<(TypeIdx, u32)>()
                    + index.changed.len() * size_of::<u32>()
            })) as u64
    }

    fn record(&self, r: u32) -> RawClassDef {
        RawClassDef::read(self.raw_bytes(), self.off + r as usize * RECORD_SIZE)
    }

    pub(crate) fn raw_bytes(&self) -> &[u8] {
        self.raw
            .as_ref()
            .expect("raw entries exist only while the buffer is retained")
            .as_bytes()
    }
}
