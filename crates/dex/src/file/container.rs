// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use crate::error::Result;
use crate::file::DexFile;
use crate::types::header::{DexHeader, DexVersion, ParseOptions};
use crate::write::compact::{
    compact_tables, has_overflowed, is_near_full, transplant_class, TableSnapshot,
};

/// Where a DEX sits after [`MultiDexContainer::redistribute_if_needed`]: its
/// group, and its index before, or `None` when its group was rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redistributed {
    pub group: usize,
    pub previous: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MultiDexContainer {
    pub dex_files: Vec<DexFile>,
}

/// How much class-data IR is currently materialized across all DEXes. Used to
/// attribute apply-phase memory to decoded instructions vs everything else.
pub use reseam_model::{MaterializationStats, MemoryBreakdown};

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

    /// Rebalances classes when a pool overflowed. `groups[i]` is the group of
    /// DEX `i`, such as the APK component it ships in; classes move only between
    /// DEX files of one group, and only groups holding an overflowed DEX are
    /// rebuilt, which materializes their classes. Returns where every DEX came
    /// from afterwards, or `None` when nothing overflowed.
    pub fn redistribute_if_needed(
        &mut self,
        groups: &[usize],
    ) -> Result<Option<Vec<Redistributed>>> {
        if !self.needs_redistribute() {
            return Ok(None);
        }
        let overflowed: HashSet<usize> = self
            .dex_files
            .iter()
            .zip(groups)
            .filter(|(dex, _)| has_overflowed(dex))
            .map(|(_, &group)| group)
            .collect();
        let mut order: Vec<usize> = Vec::new();
        for &group in groups {
            if !order.contains(&group) {
                order.push(group);
            }
        }
        let mut old: Vec<Option<DexFile>> = std::mem::take(&mut self.dex_files)
            .into_iter()
            .map(Some)
            .collect();
        let mut layout = Vec::with_capacity(old.len());
        for group in order {
            let members: Vec<usize> = (0..groups.len()).filter(|&i| groups[i] == group).collect();
            if overflowed.contains(&group) {
                let mut dexes: Vec<DexFile> = members
                    .iter()
                    .map(|&i| old[i].take().expect("each DEX is in one group"))
                    .collect();
                for dex in &mut dexes {
                    dex.resolve_all_class_data()?;
                }
                for dex in rebalance(dexes)? {
                    self.dex_files.push(dex);
                    layout.push(Redistributed {
                        group,
                        previous: None,
                    });
                }
            } else {
                for i in members {
                    self.dex_files
                        .push(old[i].take().expect("each DEX is in one group"));
                    layout.push(Redistributed {
                        group,
                        previous: Some(i),
                    });
                }
            }
        }
        Ok(Some(layout))
    }

    pub fn needs_redistribute(&self) -> bool {
        self.dex_files.iter().any(has_overflowed)
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
                .map(|raw| raw.as_bytes().len() as u64)
                .unwrap_or(0);
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
        let dex = match self.dex_files.get_mut(index) {
            Some(dex) => dex,
            None => return Ok(None),
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

/// Flattens the classes of `old_dexes`, then packs them into new DEX files so
/// that no single DEX exceeds the 64Ki pool size limit.
fn rebalance(mut old_dexes: Vec<DexFile>) -> Result<Vec<DexFile>> {
    let version = old_dexes
        .first()
        .map(|d| d.header.version)
        .unwrap_or(DexVersion::V035);

    let mut all_classes: Vec<(usize, crate::types::class::ClassDef)> = Vec::new();
    for (i, dex) in old_dexes.iter_mut().enumerate() {
        let classes = std::mem::take(&mut dex.classes).into_defs(&dex.parse_options)?;
        all_classes.extend(classes.into_iter().map(|class| (i, class)));
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
        compact_tables(dex)?;
    }
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::access_flags::AccessFlags;
    use crate::types::class::ClassDef;
    use crate::write::MAX_POOL_SIZE;

    fn dex_with_class(descriptor: &str, filler_types: usize) -> DexFile {
        let mut dex = DexFile::new(empty_header(DexVersion::V035));
        let class_type = dex.intern_type(descriptor);
        let object = dex.intern_type("Ljava/lang/Object;");
        for i in 0..filler_types {
            dex.intern_type(&format!("Lfiller/T{i};"));
        }
        dex.add_class(ClassDef {
            class_type,
            access_flags: AccessFlags::PUBLIC,
            superclass: Some(object),
            interfaces: Default::default(),
            source_file: None,
            annotations: None,
            class_data: None,
            static_values: Vec::new(),
        });
        dex
    }

    #[test]
    fn only_overflowed_groups_are_rebuilt_and_classes_stay_in_their_group() {
        let mut container = MultiDexContainer::new();
        container.add_dex(dex_with_class("LBase;", MAX_POOL_SIZE));
        container.add_dex(dex_with_class("LSplit;", 0));

        let layout = container
            .redistribute_if_needed(&[0, 1])
            .unwrap()
            .expect("the base DEX overflowed");

        assert_eq!(
            layout,
            vec![
                Redistributed {
                    group: 0,
                    previous: None
                },
                Redistributed {
                    group: 1,
                    previous: Some(1)
                },
            ]
        );
        assert_eq!(container.dex_files[0].classes.len(), 1);
        assert!(!has_overflowed(&container.dex_files[0]));
        assert_eq!(container.dex_files[1].types.len(), 2);
    }

    #[test]
    fn nothing_is_rebuilt_without_an_overflow() {
        let mut container = MultiDexContainer::new();
        container.add_dex(dex_with_class("LBase;", 0));
        container.add_dex(dex_with_class("LSplit;", 0));

        assert_eq!(container.redistribute_if_needed(&[0, 1]).unwrap(), None);
        assert_eq!(container.dex_files.len(), 2);
    }
}
