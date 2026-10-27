// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::iter::Sum;
use std::ops::Add;

use crate::file::DexFile;

/// How much class-data IR is currently materialized. Used to attribute
/// apply-phase memory to decoded instructions vs everything else.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaterializationStats {
    pub total_classes: u64,
    pub resolved_classes: u64,
    pub methods: u64,
    pub instructions: u64,
}

/// Heap estimates exclude allocation overhead and unused capacity; mapped bytes
/// describe the source span, not how many of its pages are currently resident.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryBreakdown {
    pub raw_buffer_bytes: u64,
    pub string_pool_bytes: u64,
    pub string_count: u64,
    pub id_table_bytes: u64,
    pub class_def_bytes: u64,
    pub materialized: MaterializationStats,
}

impl DexFile {
    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        MemoryBreakdown {
            raw_buffer_bytes: self
                .raw
                .as_ref()
                .map_or(0, |raw| raw.as_bytes().len() as u64),
            string_pool_bytes: self.strings.heap_bytes(),
            string_count: self.strings.len() as u64,
            id_table_bytes: self.types.heap_bytes()
                + self.prototypes.heap_bytes()
                + self.fields.heap_bytes()
                + self.methods.heap_bytes(),
            class_def_bytes: self.classes.heap_bytes() + self.ref_index_heap_bytes(),
            materialized: self.materialization_stats(),
        }
    }

    pub fn materialization_stats(&self) -> MaterializationStats {
        let mut stats = MaterializationStats {
            total_classes: self.classes.len() as u64,
            ..MaterializationStats::default()
        };
        for data in (0..self.classes.len()).filter_map(|class_idx| {
            self.classes
                .resident(class_idx)
                .and_then(|class| class.class_data.as_deref())
        }) {
            stats.resolved_classes += 1;
            for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                stats.methods += 1;
                if let Some(code) = &method.code {
                    stats.instructions += code.instructions.len() as u64;
                }
            }
        }
        stats
    }
}

impl Add for MaterializationStats {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            total_classes: self.total_classes + other.total_classes,
            resolved_classes: self.resolved_classes + other.resolved_classes,
            methods: self.methods + other.methods,
            instructions: self.instructions + other.instructions,
        }
    }
}

impl Add for MemoryBreakdown {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            raw_buffer_bytes: self.raw_buffer_bytes + other.raw_buffer_bytes,
            string_pool_bytes: self.string_pool_bytes + other.string_pool_bytes,
            string_count: self.string_count + other.string_count,
            id_table_bytes: self.id_table_bytes + other.id_table_bytes,
            class_def_bytes: self.class_def_bytes + other.class_def_bytes,
            materialized: self.materialized + other.materialized,
        }
    }
}

impl Sum for MaterializationStats {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

impl Sum for MemoryBreakdown {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

/// Lower bound on heap held by materialized IR (ignores Vec overhead,
/// tries, and debug info). Uses live type sizes so it tracks layout changes.
pub fn estimated_ir_bytes(stats: &MaterializationStats) -> u64 {
    use crate::instruction::Instruction;
    use crate::model::class::{ClassData, EncodedMethod};
    use std::mem::size_of;

    stats.instructions * size_of::<Instruction>() as u64
        + stats.methods * size_of::<EncodedMethod>() as u64
        + stats.resolved_classes * size_of::<ClassData>() as u64
}
