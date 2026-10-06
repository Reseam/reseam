// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::Result;
use crate::file::DexFile;
use crate::types::header::ParseOptions;

#[derive(Debug, Clone)]
pub struct MultiDexContainer {
    pub dex_files: Vec<DexFile>,
}

/// How much class-data IR is currently materialized across all DEXes. Used to
/// attribute apply-phase memory to decoded instructions vs everything else.
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

impl MultiDexContainer {
    pub fn new() -> Self {
        Self {
            dex_files: Vec::new(),
        }
    }

    pub fn parse(buffers: &[&[u8]], opts: ParseOptions) -> Result<Self> {
        use rayon::prelude::*;

        let results: Result<Vec<_>> = buffers
            .par_iter()
            .map(|buf| crate::read::parse::parse(buf, opts))
            .collect();
        Ok(Self {
            dex_files: results?,
        })
    }

    pub fn parse_container(buf: &[u8], opts: ParseOptions) -> Result<Self> {
        let dex_files = crate::read::parse::parse_container(buf, opts)?;
        Ok(Self { dex_files })
    }

    pub fn write_container(&self) -> Result<Vec<u8>> {
        crate::write::write_container(&self.dex_files)
    }

    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        let mut breakdown = MemoryBreakdown::default();
        for dex in &self.dex_files {
            breakdown.raw_buffer_bytes += dex
                .raw
                .as_ref()
                .map_or(0, |raw| raw.as_bytes().len() as u64);
            breakdown.string_pool_bytes += dex.strings.heap_bytes();
            breakdown.string_count += dex.strings.len() as u64;
            breakdown.id_table_bytes += dex.types.heap_bytes()
                + dex.prototypes.heap_bytes()
                + dex.fields.heap_bytes()
                + dex.methods.heap_bytes();
            breakdown.class_def_bytes += dex.classes.heap_bytes() + dex.ref_filter_heap_bytes();
        }
        breakdown.materialized = self.materialization_stats();
        breakdown
    }

    pub fn materialization_stats(&self) -> MaterializationStats {
        let mut stats = MaterializationStats::default();
        for dex in &self.dex_files {
            stats.total_classes += dex.classes.len() as u64;
            for class_idx in 0..dex.classes.len() {
                let Some(data) = dex
                    .classes
                    .resident(class_idx)
                    .and_then(|c| c.class_data.as_deref())
                else {
                    continue;
                };
                stats.resolved_classes += 1;
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    stats.methods += 1;
                    if let Some(code) = &method.code {
                        stats.instructions += code.instructions.len() as u64;
                    }
                }
            }
        }
        stats
    }

    pub fn len(&self) -> usize {
        self.dex_files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dex_files.is_empty()
    }

    pub fn dex(&self, index: usize) -> Option<&DexFile> {
        self.dex_files.get(index)
    }

    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.dex_files.get_mut(index)
    }

    pub fn dex_class_resolved(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&DexFile>> {
        let Some(dex) = self.dex_files.get_mut(index) else {
            return Ok(None);
        };
        if class_idx >= dex.classes.len() {
            return Ok(None);
        }
        dex.resolve_class_data(class_idx)?;
        Ok(Some(dex))
    }

    pub fn dex_class_resolved_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        let Some(dex) = self.dex_files.get_mut(index) else {
            return Ok(None);
        };
        if class_idx >= dex.classes.len() {
            return Ok(None);
        }
        dex.class_mut(class_idx)?;
        Ok(Some(dex))
    }

    /// `(dex index, class index)` of the class with this descriptor.
    pub fn find_class(&self, descriptor: &str) -> Option<(usize, usize)> {
        self.dex_files
            .iter()
            .enumerate()
            .find_map(|(i, dex)| dex.find_class_index(descriptor).map(|c| (i, c)))
    }

    pub fn add_dex(&mut self, dex: DexFile) {
        self.dex_files.push(dex);
    }

    pub fn extend(&mut self, mut other: Self) {
        self.dex_files.append(&mut other.dex_files);
    }

    pub fn remove_dex(&mut self, index: usize) -> DexFile {
        self.dex_files.remove(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, DexFile> {
        self.dex_files.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, DexFile> {
        self.dex_files.iter_mut()
    }
}

impl Default for MultiDexContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a MultiDexContainer {
    type Item = &'a DexFile;
    type IntoIter = std::slice::Iter<'a, DexFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.dex_files.iter()
    }
}

impl<'a> IntoIterator for &'a mut MultiDexContainer {
    type Item = &'a mut DexFile;
    type IntoIter = std::slice::IterMut<'a, DexFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.dex_files.iter_mut()
    }
}

/// Lower bound on heap held by materialized IR (ignores Vec overhead,
/// tries, and debug info). Uses live type sizes so it tracks layout changes.
pub fn estimated_ir_bytes(stats: &MaterializationStats) -> u64 {
    use crate::types::class::{ClassData, EncodedMethod};
    use crate::types::instruction::Instruction;
    use std::mem::size_of;

    stats.instructions * size_of::<Instruction>() as u64
        + stats.methods * size_of::<EncodedMethod>() as u64
        + stats.resolved_classes * size_of::<ClassData>() as u64
}
