// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use crate::file::DexFile;
use crate::types::{StringIdx, TypeIdx};

pub(super) fn build_string_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> Vec<u32> {
    let mut remap = vec![u32::MAX; source.strings.len()];
    for &string_index in used {
        let value = source.string(StringIdx(string_index));
        remap[string_index as usize] = dest.intern_string(value).0;
    }
    remap
}

pub(super) fn build_type_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> Vec<u32> {
    let mut remap = vec![u32::MAX; source.types.len()];
    for &type_index in used {
        let desc = source.type_descriptor(TypeIdx(type_index));
        remap[type_index as usize] = dest.intern_type(desc).0;
    }
    remap
}

pub(super) fn build_proto_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.prototypes.len()];
    for &proto_index in used {
        let proto = &source.prototypes[proto_index as usize];
        let ret = source.type_descriptor(proto.return_type);
        let params: Vec<&str> = proto
            .parameters
            .iter()
            .map(|param| source.type_descriptor(*param))
            .collect();
        let desc = format!("({}){}", params.join(""), ret);
        remap[proto_index as usize] = dest.intern_proto(&desc)?.0 as u32;
    }
    Ok(remap)
}

pub(super) fn build_method_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.methods.len()];
    for &method_index in used {
        let method = &source.methods[method_index as usize];
        let class_desc = source.type_descriptor(method.class);
        let name = source.string(method.name);
        let proto = &source.prototypes[method.proto.0 as usize];
        let ret = source.type_descriptor(proto.return_type);
        let params: Vec<&str> = proto
            .parameters
            .iter()
            .map(|param| source.type_descriptor(*param))
            .collect();
        let proto_desc = format!("({}){}", params.join(""), ret);
        remap[method_index as usize] = dest.intern_method(class_desc, name, &proto_desc)?.0;
    }
    Ok(remap)
}

pub(super) fn build_field_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.fields.len()];
    for &field_index in used {
        let field = &source.fields[field_index as usize];
        let class_desc = source.type_descriptor(field.class);
        let name = source.string(field.name);
        let type_desc = source.type_descriptor(field.type_);
        remap[field_index as usize] = dest.intern_field(class_desc, name, type_desc)?.0;
    }
    Ok(remap)
}

pub(super) fn build_compact_remap(used: &HashSet<u32>, len: usize) -> Vec<u32> {
    let mut remap = vec![u32::MAX; len];
    let mut new_idx = 0u32;
    for index in 0..len as u32 {
        if used.contains(&index) {
            remap[index as usize] = new_idx;
            new_idx += 1;
        }
    }
    remap
}

pub(super) fn filter_indexed<T: Clone>(items: &[T], used: &HashSet<u32>) -> Vec<T> {
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| used.contains(&(*index as u32)))
        .map(|(_, item)| item.clone())
        .collect()
}
