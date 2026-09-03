// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::OnceLock;

use rayon::prelude::*;

use super::ids::{read_type_list, validate_type_list};
use super::DexBytes;
use crate::error::{index_out_of_bounds, invalid_offset, Result};
use crate::read::class::read_class_def;
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassDef, NO_INDEX};
use crate::types::header::ParseOptions;
use crate::types::{StringIdx, TypeIdx, TypeList};

const RECORD_SIZE: usize = 32;

/// The four id fields of a `class_def_item`, readable for any class without
/// materializing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassHeader {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub source_file: Option<StringIdx>,
}

impl ClassHeader {
    pub fn of(class: &ClassDef) -> Self {
        Self {
            class_type: class.class_type,
            access_flags: class.access_flags,
            superclass: class.superclass,
            source_file: class.source_file,
        }
    }
}

/// A `class_def_item` as stored in the file.
#[derive(Debug, Clone, Copy)]
pub struct RawClassDef {
    pub class_type: u32,
    pub access_flags: u32,
    pub superclass: u32,
    pub interfaces_off: u32,
    pub source_file: u32,
    pub annotations_off: u32,
    pub class_data_off: u32,
    pub static_values_off: u32,
}

impl RawClassDef {
    fn read(buf: &[u8], off: usize) -> Self {
        let word = |i: usize| {
            let o = off + i * 4;
            u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
        };
        Self {
            class_type: word(0),
            access_flags: word(1),
            superclass: word(2),
            interfaces_off: word(3),
            source_file: word(4),
            annotations_off: word(5),
            class_data_off: word(6),
            static_values_off: word(7),
        }
    }

    pub fn header(&self) -> ClassHeader {
        ClassHeader {
            class_type: TypeIdx(self.class_type),
            access_flags: AccessFlags::from_bits_retain(self.access_flags),
            superclass: (self.superclass != NO_INDEX).then_some(TypeIdx(self.superclass)),
            source_file: (self.source_file != NO_INDEX).then_some(StringIdx(self.source_file)),
        }
    }
}

#[derive(Debug, Clone)]
enum ClassSlot {
    Raw(u32),
    Resident(Box<ClassDef>),
}

/// The class table: every class starts as a 32-byte record in the file and
/// is decoded into a [`ClassDef`] only when something mutates it. Classes
/// added after parse are resident from the start.
#[derive(Debug, Clone, Default)]
pub struct ClassTable {
    raw: Option<DexBytes>,
    off: usize,
    slots: Vec<ClassSlot>,
    by_type: OnceLock<Vec<u32>>,
}

impl ClassTable {
    pub(crate) fn from_raw(raw: DexBytes, off: u32, count: u32) -> Result<Self> {
        let buf = raw.as_bytes();
        let off = off as usize;
        let count = count as usize;
        if off + count * RECORD_SIZE > buf.len() {
            return Err(invalid_offset("class_defs", off as u32, buf.len() as u32));
        }
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
            by_type: OnceLock::new(),
        })
    }

    pub fn from_defs(defs: Vec<ClassDef>) -> Self {
        Self {
            slots: defs.into_iter().map(|c| ClassSlot::Resident(Box::new(c))).collect(),
            ..Self::default()
        }
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
            ClassSlot::Resident(c) => ClassHeader::of(c),
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
            ClassSlot::Resident(c) => c.interfaces.clone(),
        }
    }

    /// The file record of a class that has not been materialized.
    pub fn raw_def(&self, i: usize) -> Option<RawClassDef> {
        match self.slots.get(i)? {
            ClassSlot::Raw(r) => Some(self.record(*r)),
            ClassSlot::Resident(_) => None,
        }
    }

    pub fn resident(&self, i: usize) -> Option<&ClassDef> {
        match self.slots.get(i)? {
            ClassSlot::Resident(c) => Some(c),
            ClassSlot::Raw(_) => None,
        }
    }

    pub fn resident_mut(&mut self, i: usize) -> Option<&mut ClassDef> {
        match self.slots.get_mut(i)? {
            ClassSlot::Resident(c) => Some(c),
            ClassSlot::Raw(_) => None,
        }
    }

    /// Every resident class, in table order. Classes still in the file are
    /// skipped; call [`Self::materialize_all`] first to see all of them.
    pub fn iter_resident(&self) -> impl Iterator<Item = &ClassDef> {
        self.slots.iter().filter_map(|s| match s {
            ClassSlot::Resident(c) => Some(&**c),
            ClassSlot::Raw(_) => None,
        })
    }

    pub fn iter_resident_mut(&mut self) -> impl Iterator<Item = &mut ClassDef> {
        self.slots.iter_mut().filter_map(|s| match s {
            ClassSlot::Resident(c) => Some(&mut **c),
            ClassSlot::Raw(_) => None,
        })
    }

    pub fn is_resident(&self, i: usize) -> bool {
        matches!(self.slots.get(i), Some(ClassSlot::Resident(_)))
    }

    pub fn all_resident(&self) -> bool {
        self.slots.iter().all(|s| matches!(s, ClassSlot::Resident(_)))
    }

    pub fn resident_count(&self) -> usize {
        self.slots.iter().filter(|s| matches!(s, ClassSlot::Resident(_))).count()
    }

    /// Decodes the class from the file if needed and returns it mutably.
    pub fn materialize(&mut self, i: usize, opts: &ParseOptions) -> Result<&mut ClassDef> {
        let len = self.len();
        let Some(slot) = self.slots.get_mut(i) else {
            return Err(index_out_of_bounds("class", i as u32, len as u32));
        };
        if let ClassSlot::Raw(r) = *slot {
            let buf = self.raw_bytes();
            let def = read_class_def(buf, RawClassDef::read(buf, self.off + r as usize * RECORD_SIZE), opts)?;
            self.slots[i] = ClassSlot::Resident(Box::new(def));
        }
        Ok(self.resident_mut(i).unwrap())
    }

    pub fn materialize_all(&mut self, opts: &ParseOptions) -> Result<()> {
        let Some(raw) = self.raw.clone() else {
            return Ok(());
        };
        let buf = raw.as_bytes();
        let off = self.off;
        self.slots.par_iter_mut().try_for_each(|slot| {
            if let ClassSlot::Raw(r) = *slot {
                let def = read_class_def(buf, RawClassDef::read(buf, off + r as usize * RECORD_SIZE), opts)?;
                *slot = ClassSlot::Resident(Box::new(def));
            }
            Ok(())
        })
    }

    pub fn push(&mut self, class: ClassDef) -> usize {
        self.slots.push(ClassSlot::Resident(Box::new(class)));
        self.by_type.take();
        self.slots.len() - 1
    }

    pub fn remove(&mut self, i: usize, opts: &ParseOptions) -> Result<ClassDef> {
        self.materialize(i, opts)?;
        self.by_type.take();
        match self.slots.remove(i) {
            ClassSlot::Resident(c) => Ok(*c),
            ClassSlot::Raw(_) => unreachable!("materialized above"),
        }
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.slots.truncate(len);
        self.by_type.take();
    }

    pub fn into_defs(mut self, opts: &ParseOptions) -> Result<Vec<ClassDef>> {
        self.materialize_all(opts)?;
        Ok(self
            .slots
            .into_iter()
            .map(|slot| match slot {
                ClassSlot::Resident(c) => *c,
                ClassSlot::Raw(_) => unreachable!("materialized above"),
            })
            .collect())
    }

    pub fn index_of_type(&self, type_idx: TypeIdx) -> Option<usize> {
        let by_type = self.by_type.get_or_init(|| {
            let mut order: Vec<u32> = (0..self.len() as u32).collect();
            order.sort_by_key(|&i| self.header(i as usize).class_type);
            order
        });
        by_type
            .binary_search_by_key(&type_idx, |&i| self.header(i as usize).class_type)
            .ok()
            .map(|pos| by_type[pos] as usize)
    }

    pub(crate) fn invalidate_index(&mut self) {
        self.by_type.take();
    }

    pub fn heap_bytes(&self) -> u64 {
        (self.slots.len() * size_of::<ClassSlot>()
            + self.resident_count() * size_of::<ClassDef>()
            + self.by_type.get().map_or(0, |v| v.len() * 4)) as u64
    }

    fn record(&self, r: u32) -> RawClassDef {
        RawClassDef::read(self.raw_bytes(), self.off + r as usize * RECORD_SIZE)
    }

    fn raw_bytes(&self) -> &[u8] {
        self.raw
            .as_ref()
            .expect("raw entries exist only while the buffer is retained")
            .as_bytes()
    }
}
