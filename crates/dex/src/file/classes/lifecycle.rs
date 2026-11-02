// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use rayon::prelude::*;

use super::{ClassKey, ClassSlot, ClassTable, ClassUndo, Prior, record_at};
use crate::error::{Result, index_out_of_bounds};
use crate::model::class::ClassDef;
use crate::options::ParseOptions;
use crate::read::class::read_class_def;

impl ClassTable {
    /// The class, decoded from the file when it is not resident. Reading never
    /// makes a class resident, so the table and its journal stay unchanged.
    pub fn read(&self, i: usize, opts: ParseOptions) -> Result<Cow<'_, ClassDef>> {
        match self.slots.get(i) {
            Some(ClassSlot::Resident(class)) => Ok(Cow::Borrowed(&class.def)),
            Some(&ClassSlot::Raw(r)) => self.decode(r, opts).map(Cow::Owned),
            None => Err(index_out_of_bounds("class", i as u32, self.len() as u32)),
        }
    }

    fn decode(&self, r: u32, opts: ParseOptions) -> Result<ClassDef> {
        read_class_def(
            self.raw.as_ref().expect("raw slot retains its source"),
            self.record(r),
            opts,
        )
    }

    /// Decodes the class from the file if needed and returns it mutably.
    pub fn materialize(&mut self, i: usize, opts: ParseOptions) -> Result<&mut ClassDef> {
        let len = self.len();
        let Some(slot) = self.slots.get_mut(i) else {
            return Err(index_out_of_bounds("class", i as u32, len as u32));
        };
        if let ClassSlot::Raw(r) = *slot {
            self.journal.record(ClassKey::Slot(i as u32), || {
                ClassUndo::Restore(i as u32, Prior::Raw(r))
            });
            let def = self.decode(r, opts)?;
            self.slots[i] = ClassSlot::resident(def);
            let pos = self.resident.partition_point(|&r| (r as usize) < i);
            self.resident.insert(pos, i as u32);
        }
        Ok(self
            .resident_mut(i)
            .expect("materialization installs a resident class"))
    }

    pub fn materialize_all(&mut self, opts: ParseOptions) -> Result<()> {
        let Some(raw) = self.raw.clone() else {
            return Ok(());
        };
        for (i, slot) in self.slots.iter().enumerate() {
            if let ClassSlot::Raw(r) = *slot {
                self.journal.record(ClassKey::Slot(i as u32), || {
                    ClassUndo::Restore(i as u32, Prior::Raw(r))
                });
            }
        }
        let off = self.off;
        self.slots
            .par_iter_mut()
            .try_for_each(|slot| -> Result<()> {
                if let ClassSlot::Raw(r) = *slot {
                    let def = read_class_def(&raw, record_at(raw.as_bytes(), off, r), opts)?;
                    *slot = ClassSlot::resident(def);
                }
                Ok(())
            })?;
        self.resident = (0..self.slots.len() as u32).collect();
        Ok(())
    }

    pub fn push(&mut self, class: ClassDef) -> usize {
        let len = self.slots.len();
        self.journal
            .record(ClassKey::Len, || ClassUndo::Truncate(len));
        self.slots.push(ClassSlot::resident(class));
        self.by_type.take();
        let i = self.slots.len() - 1;
        self.resident.push(i as u32);
        i
    }

    pub fn remove(&mut self, i: usize, opts: ParseOptions) -> Result<ClassDef> {
        self.materialize(i, opts)?;
        self.by_type.take();
        let pos = self.resident.partition_point(|&r| (r as usize) < i);
        self.resident.remove(pos);
        self.resident[pos..].iter_mut().for_each(|r| *r -= 1);
        let def = match self.slots.remove(i) {
            ClassSlot::Resident(c) => c.def,
            ClassSlot::Raw(_) => {
                unreachable!("materialization has replaced the raw slot with a resident class")
            }
        };
        self.journal
            .barrier(|| ClassUndo::Insert(i as u32, Box::new(def.clone())));
        Ok(def)
    }
}
