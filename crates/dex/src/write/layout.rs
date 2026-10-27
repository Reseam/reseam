// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::plan::WritePlan;
use crate::file::DexFile;
use crate::model::{MethodIdx, Pool, TypeIdx};

/// Where the types, methods and classes of a source [`DexFile`] landed in one
/// written DEX. Source indices are those of the in-memory pools, which keep
/// every parsed entry at its original index.
pub struct WriteLayout {
    types: Option<Vec<u32>>,
    methods: Option<Vec<u32>>,
    class_types: Vec<TypeIdx>,
    type_count: u32,
    method_count: u32,
}

impl WriteLayout {
    /// The layout of `dex` as parsed, for a DEX that is copied instead of written.
    pub fn unchanged(dex: &DexFile) -> Self {
        Self {
            types: None,
            methods: None,
            class_types: dex
                .classes
                .headers()
                .map(|class| class.class_type)
                .collect(),
            type_count: dex.types.len() as u32,
            method_count: dex.methods.len() as u32,
        }
    }

    pub(crate) fn of(plan: WritePlan<'_>) -> Self {
        let type_count = plan.count(Pool::Type) as u32;
        let method_count = plan.count(Pool::Method) as u32;
        let class_types = plan
            .class_order
            .iter()
            .map(|&class| plan.dex.classes.header(class).class_type)
            .collect();
        let (types, methods) = plan
            .remap
            .map(|remap| {
                let mut tables = remap.into_tables();
                (
                    std::mem::take(&mut tables[Pool::Type]),
                    std::mem::take(&mut tables[Pool::Method]),
                )
            })
            .unzip();
        Self {
            types,
            methods,
            class_types,
            type_count,
            method_count,
        }
    }

    pub fn type_index(&self, source: TypeIdx) -> Option<TypeIdx> {
        remapped(self.types.as_deref(), source.0, self.type_count).map(TypeIdx)
    }

    pub fn method_index(&self, source: MethodIdx) -> Option<MethodIdx> {
        remapped(self.methods.as_deref(), source.0, self.method_count).map(MethodIdx)
    }

    /// Source type of each written class definition, in output order.
    pub fn class_types(&self) -> &[TypeIdx] {
        &self.class_types
    }

    pub fn type_count(&self) -> u32 {
        self.type_count
    }

    pub fn method_count(&self) -> u32 {
        self.method_count
    }
}

fn remapped(remap: Option<&[u32]>, source: u32, count: u32) -> Option<u32> {
    match remap {
        Some(remap) => remap
            .get(source as usize)
            .copied()
            .filter(|&i| i != u32::MAX),
        None => (source < count).then_some(source),
    }
}
