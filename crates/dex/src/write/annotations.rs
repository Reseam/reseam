// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::encoded_value::write_encoded_value;
use super::intern::{ByteInterner, StreamInterner};
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::encoding::leb128::write_uleb128;
use crate::error::Result;
use crate::model::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::model::map::{
    TYPE_ANNOTATION_ITEM, TYPE_ANNOTATION_SET_ITEM, TYPE_ANNOTATION_SET_REF_LIST,
    TYPE_ANNOTATIONS_DIRECTORY_ITEM,
};

/// A class's annotations directory, its sets and lists named by interned index.
struct Directory {
    class: Option<usize>,
    fields: Vec<(u32, usize)>,
    methods: Vec<(u32, usize)>,
    parameters: Vec<(u32, usize)>,
}

/// Annotation items, sets and parameter lists, each written once however
/// many members share it. Items stream out as they are first seen; sets and
/// lists are written after them, since they point at items.
struct AnnotationInterner {
    items: StreamInterner,
    sets: ByteInterner,
    ref_lists: Vec<Vec<Option<usize>>>,
    encoded: Vec<u8>,
}

impl AnnotationInterner {
    fn directory<S: DexSink>(
        &mut self,
        w: &mut DexWriter<S>,
        dir: &AnnotationsDirectory,
    ) -> Result<Directory> {
        let class = (!dir.class.is_empty())
            .then(|| self.set(w, &dir.class))
            .transpose()?;
        let fields = dir
            .fields
            .iter()
            .map(|(field, items)| Ok((field.0, self.set(w, items)?)))
            .collect::<Result<_>>()?;
        let methods = dir
            .methods
            .iter()
            .map(|(method, items)| Ok((method.0, self.set(w, items)?)))
            .collect::<Result<_>>()?;
        let mut parameters = Vec::with_capacity(dir.parameters.len());
        for (method, params) in &dir.parameters {
            let list = params
                .iter()
                .map(|items| (!items.is_empty()).then(|| self.set(w, items)).transpose())
                .collect::<Result<_>>()?;
            parameters.push((method.0, self.ref_lists.len()));
            self.ref_lists.push(list);
        }
        Ok(Directory {
            class,
            fields,
            methods,
            parameters,
        })
    }

    fn set<S: DexSink>(&mut self, w: &mut DexWriter<S>, items: &[AnnotationItem]) -> Result<usize> {
        let mut key = Vec::with_capacity(items.len() * 4);
        for item in items {
            self.encoded.clear();
            self.encoded.push(item.visibility.to_u8());
            write_uleb128(&mut self.encoded, item.type_.0);
            write_uleb128(&mut self.encoded, item.elements.len() as u32);
            for element in &item.elements {
                write_uleb128(&mut self.encoded, element.name.0);
                write_encoded_value(&mut self.encoded, &element.value);
            }
            let offset = self.items.intern(&mut w.sink, &self.encoded)?;
            key.extend_from_slice(&offset.to_le_bytes());
        }
        self.sets.intern(&key)
    }
}

/// Writes every class's annotations, returning each class's directory
/// offset, zero for a class without one.
pub(crate) fn write_annotations<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<Vec<u32>> {
    let items_start = w.pos();
    let mut interner = AnnotationInterner {
        items: StreamInterner::default(),
        sets: ByteInterner::new()?,
        ref_lists: Vec::new(),
        encoded: Vec::new(),
    };
    let directories = (0..plan.classes.len())
        .map(|k| {
            plan.class_annotations(k)?
                .map(|dir| interner.directory(w, &dir))
                .transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    w.map_section(
        TYPE_ANNOTATION_ITEM,
        items_start,
        interner.items.len() as u32,
    );
    let sets = write_sets(w, &interner.sets)?;
    let ref_lists = write_ref_lists(w, &interner.ref_lists, &sets);
    Ok(write_directories(w, directories, &sets, &ref_lists))
}

fn write_sets<S: DexSink>(w: &mut DexWriter<S>, sets: &ByteInterner) -> Result<Vec<u32>> {
    if sets.is_empty() {
        return Ok(Vec::new());
    }
    w.align(4);
    let start = w.pos();
    let mut offsets = Vec::with_capacity(sets.len());
    let mut set = Vec::new();
    for index in 0..sets.len() {
        sets.get(index, &mut set)?;
        offsets.push(w.pos());
        w.write_u32((set.len() / 4) as u32);
        w.write(&set);
    }
    w.map_section(TYPE_ANNOTATION_SET_ITEM, start, sets.len() as u32);
    Ok(offsets)
}

fn write_ref_lists<S: DexSink>(
    w: &mut DexWriter<S>,
    lists: &[Vec<Option<usize>>],
    sets: &[u32],
) -> Vec<u32> {
    if lists.is_empty() {
        return Vec::new();
    }
    w.align(4);
    let start = w.pos();
    let offsets = lists
        .iter()
        .map(|list| {
            let offset = w.pos();
            w.write_u32(list.len() as u32);
            for set in list {
                w.write_u32(set.map_or(0, |set| sets[set]));
            }
            offset
        })
        .collect();
    w.map_section(TYPE_ANNOTATION_SET_REF_LIST, start, lists.len() as u32);
    offsets
}

fn write_directories<S: DexSink>(
    w: &mut DexWriter<S>,
    directories: Vec<Option<Directory>>,
    sets: &[u32],
    ref_lists: &[u32],
) -> Vec<u32> {
    let count = directories.iter().flatten().count();
    if count == 0 {
        return vec![0; directories.len()];
    }
    w.align(4);
    let start = w.pos();
    let offsets = directories
        .into_iter()
        .map(|directory| {
            let Some(directory) = directory else {
                return 0;
            };
            let offset = w.pos();
            w.write_u32(directory.class.map_or(0, |set| sets[set]));
            w.write_u32(directory.fields.len() as u32);
            w.write_u32(directory.methods.len() as u32);
            w.write_u32(directory.parameters.len() as u32);
            for (member, set) in directory.fields.into_iter().chain(directory.methods) {
                w.write_u32(member);
                w.write_u32(sets[set]);
            }
            for (member, list) in directory.parameters {
                w.write_u32(member);
                w.write_u32(ref_lists[list]);
            }
            offset
        })
        .collect();
    w.map_section(TYPE_ANNOTATIONS_DIRECTORY_ITEM, start, count as u32);
    offsets
}
