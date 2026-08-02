// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::file::DexFile;
use crate::types::header::{DexHeader, DexVersion, ParseOptions};
use crate::write::compact::{
    compact_tables, has_overflowed, is_near_full, transplant_class, TableSnapshot,
};

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

impl MaterializationStats {
    /// Lower bound on heap held by materialized IR (ignores Vec overhead,
    /// tries, and debug info). Uses live type sizes so it tracks layout changes.
    pub fn estimated_ir_bytes(&self) -> u64 {
        use crate::types::class::{ClassData, EncodedMethod};
        use crate::types::instruction::Instruction;
        use std::mem::size_of;

        self.instructions * size_of::<Instruction>() as u64
            + self.methods * size_of::<EncodedMethod>() as u64
            + self.resolved_classes * size_of::<ClassData>() as u64
    }
}

/// Full native heap attribution for a container, so RSS can be split into its
/// contributors rather than guessed at. All figures are lower bounds (they
/// exclude `Vec` capacity slack and allocator overhead).
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

        let results: std::result::Result<Vec<_>, _> = buffers
            .par_iter()
            .map(|buf| crate::read::parse::parse(buf, opts.clone()))
            .collect();
        Ok(Self {
            dex_files: results?,
        })
    }

    pub fn parse_container(buf: &[u8], opts: ParseOptions) -> Result<Self> {
        let dex_files = crate::read::parse::parse_container(buf, opts)?;
        Ok(Self { dex_files })
    }

    pub fn write_all(&mut self) -> Result<Vec<Vec<u8>>> {
        for dex in &mut self.dex_files {
            dex.resolve_all_class_data()?;
        }

        let needs_redistribute = self.needs_redistribute();
        if needs_redistribute {
            self.redistribute()?;
        }

        use rayon::prelude::*;
        self.dex_files
            .par_iter_mut()
            .map(crate::write::write)
            .collect()
    }

    pub fn needs_redistribute(&self) -> bool {
        self.dex_files.iter().any(has_overflowed)
    }

    /// Flatten all classes from all DEX files, then redistribute them across
    /// new DEX files so that no single DEX exceeds the 64Ki pool size limit.
    fn redistribute(&mut self) -> Result<()> {
        let mut old_dexes = std::mem::take(&mut self.dex_files);
        let version = old_dexes
            .first()
            .map(|d| d.header.version)
            .unwrap_or(DexVersion::V035);

        let mut all_classes: Vec<(usize, crate::types::class::ClassDef)> = Vec::new();
        for (i, dex) in old_dexes.iter_mut().enumerate() {
            for class in dex.classes.drain(..) {
                all_classes.push((i, class));
            }
        }

        let mut output: Vec<DexFile> = Vec::new();
        let mut current = DexFile::new(empty_header(version));

        for (src_idx, mut class) in all_classes {
            let source = &old_dexes[src_idx];

            if is_near_full(&current) {
                let snap = TableSnapshot::capture(&current);
                let class_backup = class.clone();

                transplant_class(&mut class, source, &mut current)?;
                current.add_class(class);

                if has_overflowed(&current) {
                    snap.restore(&mut current);

                    if !current.classes.is_empty() {
                        output.push(current);
                    }
                    current = DexFile::new(empty_header(version));

                    class = class_backup;
                    transplant_class(&mut class, source, &mut current)?;
                    current.add_class(class);
                }
            } else {
                transplant_class(&mut class, source, &mut current)?;
                current.add_class(class);
            }
        }

        if !current.classes.is_empty() {
            output.push(current);
        }

        for dex in &mut output {
            compact_tables(dex);
        }
        self.dex_files = output;
        Ok(())
    }

    pub fn write_container(&mut self) -> Result<Vec<u8>> {
        crate::write::write_container(&mut self.dex_files)
    }

    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        use crate::types::class::ClassDef;
        use crate::types::{DexString, FieldId, MethodId, Prototype, StringIdx};
        use std::mem::size_of;

        let mut breakdown = MemoryBreakdown::default();
        for dex in &self.dex_files {
            breakdown.raw_buffer_bytes += dex
                .raw
                .as_ref()
                .map(|raw| raw.as_bytes().len() as u64)
                .unwrap_or(0);
            for string in &dex.strings {
                breakdown.string_pool_bytes += string.value.len() as u64;
            }
            breakdown.string_pool_bytes += dex.strings.len() as u64 * size_of::<DexString>() as u64;
            breakdown.string_count += dex.strings.len() as u64;
            breakdown.id_table_bytes += dex.types.len() as u64 * size_of::<StringIdx>() as u64
                + dex.prototypes.len() as u64 * size_of::<Prototype>() as u64
                + dex.fields.len() as u64 * size_of::<FieldId>() as u64
                + dex.methods.len() as u64 * size_of::<MethodId>() as u64;
            breakdown.class_def_bytes += dex.classes.len() as u64 * size_of::<ClassDef>() as u64;
        }
        breakdown.materialized = self.materialization_stats();
        breakdown
    }

    pub fn materialization_stats(&self) -> MaterializationStats {
        let mut stats = MaterializationStats::default();
        for dex in &self.dex_files {
            stats.total_classes += dex.classes.len() as u64;
            for class in &dex.classes {
                let Some(data) = &class.class_data else {
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
        self.dex_class_resolved_mut(index, class_idx)
            .map(|opt| opt.map(|dex| &*dex))
    }

    pub fn dex_class_resolved_mut(
        &mut self,
        index: usize,
        class_idx: usize,
    ) -> Result<Option<&mut DexFile>> {
        let dex = match self.dex_files.get_mut(index) {
            Some(dex) => dex,
            None => return Ok(None),
        };
        if class_idx >= dex.classes.len() {
            return Ok(None);
        }
        if dex.classes[class_idx].class_data.is_none() {
            dex.resolve_class_data(class_idx)?;
        }
        Ok(Some(dex))
    }

    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &crate::types::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter().enumerate() {
            if let Some(class) = dex.find_class(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    pub fn find_class_mut(
        &mut self,
        descriptor: &str,
    ) -> Option<(usize, &mut crate::types::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter_mut().enumerate() {
            if let Some(class) = dex.find_class_mut(descriptor) {
                return Some((i, class));
            }
        }
        None
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

fn empty_header(version: DexVersion) -> DexHeader {
    DexHeader {
        version,
        checksum: 0,
        signature: [0; 20],
        file_size: 0,
        link_size: 0,
        link_off: 0,
        map_off: 0,
        string_ids_size: 0,
        string_ids_off: 0,
        type_ids_size: 0,
        type_ids_off: 0,
        proto_ids_size: 0,
        proto_ids_off: 0,
        field_ids_size: 0,
        field_ids_off: 0,
        method_ids_size: 0,
        method_ids_off: 0,
        class_defs_size: 0,
        class_defs_off: 0,
        data_size: 0,
        data_off: 0,
        container_size: 0,
        header_offset: 0,
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
