// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod class;
mod pools;
mod written;

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rustc_hash::FxHashMap;

use super::part::DexPart;
use super::remap::Remap;
use crate::error::{Result, invalid};
use crate::file::DexFile;
use crate::model::annotation::AnnotationsDirectory;
use crate::model::class::{ClassDef, ClassHeader};
use crate::model::encoded_value::EncodedValue;
use crate::model::{PerPool, TypeList};
use crate::read::annotation::read_annotations_directory;
use crate::read::class::RawClassDef;

pub(crate) enum WriteClass<'a> {
    Resident(&'a ClassDef),
    Raw(RawClassDef),
}

pub(crate) struct WritePlan<'a> {
    pub dex: &'a DexFile,
    pub options: crate::options::WriteOptions,
    /// `None` when every pool is written whole and already in DEX sort order.
    pub order: Option<PerPool<Vec<u32>>>,
    pub remap: Option<Remap>,
    pub classes: Vec<WriteClass<'a>>,
    /// Source class-table index for each output class, including class-indexed metadata.
    pub class_order: Vec<usize>,
}

impl<'a> WritePlan<'a> {
    pub(crate) fn new(
        dex: &'a DexFile,
        part: Option<&DexPart>,
        options: crate::options::WriteOptions,
    ) -> Result<Self> {
        crate::references::validate_pools(dex)?;
        if let Some(part) = part {
            crate::references::validate_part(dex, part)?;
        }
        let (order, remap) = pools::order_pools(dex, part).unzip();

        let mut plan = Self {
            dex,
            options,
            order,
            remap,
            classes: Vec::new(),
            class_order: Vec::new(),
        };
        plan.select_classes(part)?;
        Ok(plan)
    }

    fn select_classes(&mut self, part: Option<&DexPart>) -> Result<()> {
        let dex = self.dex;
        let sources: Vec<usize> = match part {
            Some(part) => part.classes.clone(),
            None => (0..dex.classes.len()).collect(),
        };
        let mut classes: Vec<WriteClass<'_>> = Vec::with_capacity(sources.len());
        for &i in &sources {
            if i >= dex.classes.len() {
                return Err(invalid(
                    "DEX part",
                    "class index exceeds the source class table",
                ));
            }
            if let Some(class) = dex.classes.resident(i) {
                crate::references::validate_class(dex, class)?;
            }
            classes.push(match (dex.classes.resident(i), dex.classes.raw_def(i)) {
                (Some(class), _) => WriteClass::Resident(class),
                (None, Some(raw)) => WriteClass::Raw(raw),
                (None, None) => {
                    return Err(invalid(
                        "DEX class",
                        format!("class index {i} has no source"),
                    ));
                }
            });
        }
        self.classes = classes;
        let positions = self.order_classes()?;
        let mut slots: Vec<_> = self.classes.drain(..).map(Some).collect();
        for &position in &positions {
            self.classes.push(
                slots[position]
                    .take()
                    .expect("class order is a permutation"),
            );
        }
        self.class_order = positions.into_iter().map(|p| sources[p]).collect();
        Ok(())
    }

    fn order_classes(&self) -> Result<Vec<usize>> {
        let count = self.classes.len();
        let mut index_of = FxHashMap::default();
        for i in 0..count {
            let type_idx = self.class_header(i).class_type;
            if index_of.insert(type_idx, i).is_some() {
                return Err(invalid(
                    "class_defs",
                    format!("duplicate class type index {}", type_idx.0),
                ));
            }
        }

        let mut dependents = vec![Vec::new(); count];
        let mut pending = vec![0usize; count];
        for (i, dependencies) in pending.iter_mut().enumerate() {
            let interfaces = self.class_interfaces(i);
            for dependency in self
                .class_header(i)
                .superclass
                .into_iter()
                .chain(interfaces)
            {
                // A superclass or interface in another DEX has no ordering
                // constraint in this file.
                if let Some(&parent) = index_of.get(&dependency) {
                    dependents[parent].push(i);
                    *dependencies += 1;
                }
            }
        }

        let mut ready: BinaryHeap<_> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(Reverse(i)))
            .collect();
        let mut order = Vec::with_capacity(count);
        while let Some(Reverse(i)) = ready.pop() {
            order.push(i);
            for &child in &dependents[i] {
                pending[child] -= 1;
                if pending[child] == 0 {
                    ready.push(Reverse(child));
                }
            }
        }
        if order.len() != count {
            return Err(invalid(
                "class_defs",
                "cycle in superclass/interface dependencies",
            ));
        }
        Ok(order)
    }

    pub(crate) fn remap(&self) -> Option<&Remap> {
        self.remap.as_ref()
    }

    pub(crate) fn raw_bytes(&self) -> &'a [u8] {
        self.dex
            .raw
            .as_ref()
            .expect("file classes exist only while the buffer is retained")
            .as_bytes()
    }
}
