// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use crate::file::DexFile;
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;
use crate::types::method_handle::{
    CallSiteItem, MethodHandle, MethodHandleIdx, MethodHandleMember,
};

use super::sort::Remap;
use super::MAX_POOL_SIZE;
use collect::ReferencedIndices;
use exotic::{remap_all_exotic_indices, remap_exotic_refs};
use remap::{
    build_compact_remap, build_field_remap, build_method_remap, build_proto_remap,
    build_string_remap, build_type_remap, filter_indexed,
};

mod collect;
mod exotic;
mod remap;

/// Transplant a single class from `source` DEX into `dest` DEX.
///
/// Interns all referenced strings/types/protos/methods/fields into dest,
/// remaps the class's indices to dest's tables, and handles call_sites
/// and method_handles.
///
/// The class is modified in-place with remapped indices. The caller must
/// add it to `dest.classes` after this returns.
pub(crate) fn transplant_class(
    class: &mut ClassDef,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<()> {
    let mut refs = ReferencedIndices::collect_from_classes(std::slice::from_ref(class));
    refs.expand_transitive(source);

    let string_remap = build_string_remap(&refs.strings, source, dest);
    let type_remap = build_type_remap(&refs.types, source, dest);
    let proto_remap = build_proto_remap(&refs.protos, source, dest)?;
    let method_remap = build_method_remap(&refs.methods, source, dest)?;
    let field_remap = build_field_remap(&refs.fields, source, dest)?;

    let mut mh_remap: HashMap<u32, u32> = HashMap::new();
    for &mh_idx in &refs.method_handles {
        if let Some(handle) = source.method_handles.get(mh_idx as usize) {
            let new_member = match &handle.member {
                MethodHandleMember::Field(idx) => {
                    MethodHandleMember::Field(crate::types::FieldIdx(field_remap[idx.0 as usize]))
                }
                MethodHandleMember::Method(idx) => MethodHandleMember::Method(
                    crate::types::MethodIdx(method_remap[idx.0 as usize]),
                ),
            };
            let remapped = MethodHandle {
                handle_type: handle.handle_type,
                member: new_member,
            };
            let new_idx =
                if let Some(pos) = dest.method_handles.iter().position(|mh| *mh == remapped) {
                    pos as u32
                } else {
                    let idx = dest.method_handles.len() as u32;
                    dest.method_handles.push(remapped);
                    idx
                };
            mh_remap.insert(mh_idx, new_idx);
        }
    }

    let mut cs_remap: HashMap<u32, u32> = HashMap::new();
    for &cs_idx in &refs.call_sites {
        if let Some(cs) = source.call_sites.get(cs_idx as usize) {
            let bootstrap = mh_remap
                .get(&cs.bootstrap_method.0)
                .map(|&value| MethodHandleIdx(value))
                .unwrap_or(cs.bootstrap_method);

            let remap = Remap {
                string: &string_remap,
                type_: &type_remap,
                proto: &proto_remap,
                field: &field_remap,
                method: &method_remap,
            };

            let mut new_cs = CallSiteItem {
                bootstrap_method: bootstrap,
                method_name: remap.remap_string(cs.method_name),
                method_type: remap.remap_proto(cs.method_type),
                extra_arguments: cs.extra_arguments.clone(),
            };

            for arg in &mut new_cs.extra_arguments {
                remap_encoded_value_full(arg, &remap, &mh_remap);
            }

            let new_idx = if let Some(pos) = dest
                .call_sites
                .iter()
                .position(|existing| *existing == new_cs)
            {
                pos as u32
            } else {
                let idx = dest.call_sites.len() as u32;
                dest.call_sites.push(new_cs);
                idx
            };
            cs_remap.insert(cs_idx, new_idx);
        }
    }

    let remap = Remap {
        string: &string_remap,
        type_: &type_remap,
        proto: &proto_remap,
        field: &field_remap,
        method: &method_remap,
    };
    remap.remap_class(class);

    if !cs_remap.is_empty() || !mh_remap.is_empty() {
        remap_exotic_refs(class, &cs_remap, &mh_remap);
    }

    Ok(())
}

fn remap_encoded_value_full(v: &mut EncodedValue, remap: &Remap<'_>, mh_remap: &HashMap<u32, u32>) {
    match v {
        EncodedValue::String(idx) => *idx = remap.remap_string(*idx),
        EncodedValue::Type(idx) => *idx = remap.remap_type(*idx),
        EncodedValue::Field(idx) => *idx = remap.remap_field(*idx),
        EncodedValue::Method(idx) => *idx = remap.remap_method(*idx),
        EncodedValue::Enum(idx) => *idx = remap.remap_field(*idx),
        EncodedValue::MethodType(idx) => *idx = remap.remap_proto(*idx),
        EncodedValue::MethodHandle(idx) => {
            if let Some(&new_mh) = mh_remap.get(&idx.0) {
                *idx = MethodHandleIdx(new_mh);
            }
        }
        EncodedValue::Array(items) => {
            for item in items {
                remap_encoded_value_full(item, remap, mh_remap);
            }
        }
        EncodedValue::Annotation(ann) => {
            ann.type_ = remap.remap_type(ann.type_);
            for elem in &mut ann.elements {
                elem.name = remap.remap_string(elem.name);
                remap_encoded_value_full(&mut elem.value, remap, mh_remap);
            }
        }
        _ => {}
    }
}

pub(crate) struct TableSnapshot {
    strings: usize,
    types: usize,
    protos: usize,
    fields: usize,
    methods: usize,
    classes: usize,
    call_sites: usize,
    method_handles: usize,
}

impl TableSnapshot {
    pub(crate) fn capture(dex: &DexFile) -> Self {
        Self {
            strings: dex.strings.len(),
            types: dex.types.len(),
            protos: dex.prototypes.len(),
            fields: dex.fields.len(),
            methods: dex.methods.len(),
            classes: dex.classes.len(),
            call_sites: dex.call_sites.len(),
            method_handles: dex.method_handles.len(),
        }
    }

    pub(crate) fn restore(self, dex: &mut DexFile) {
        dex.strings.truncate(self.strings);
        dex.types.truncate(self.types);
        dex.prototypes.truncate(self.protos);
        dex.fields.truncate(self.fields);
        dex.methods.truncate(self.methods);
        dex.classes.truncate(self.classes);
        dex.call_sites.truncate(self.call_sites);
        dex.method_handles.truncate(self.method_handles);
        dex.invalidate_lookups();
    }
}

pub(crate) fn has_overflowed(dex: &DexFile) -> bool {
    dex.types.len() > MAX_POOL_SIZE
        || dex.prototypes.len() > MAX_POOL_SIZE
        || dex.fields.len() > MAX_POOL_SIZE
        || dex.methods.len() > MAX_POOL_SIZE
        || dex.call_sites.len() > MAX_POOL_SIZE
        || dex.method_handles.len() > MAX_POOL_SIZE
}

pub(crate) fn is_near_full(dex: &DexFile) -> bool {
    const MARGIN: usize = 500;
    dex.types.len() + MARGIN > MAX_POOL_SIZE
        || dex.prototypes.len() + MARGIN > MAX_POOL_SIZE
        || dex.fields.len() + MARGIN > MAX_POOL_SIZE
        || dex.methods.len() + MARGIN > MAX_POOL_SIZE
        || dex.call_sites.len() + MARGIN > MAX_POOL_SIZE
        || dex.method_handles.len() + MARGIN > MAX_POOL_SIZE
}

pub(crate) fn compact_tables(dex: &mut DexFile) {
    let refs = ReferencedIndices::collect_from_dex(dex);

    let nothing_to_compact = refs.strings.len() == dex.strings.len()
        && refs.types.len() == dex.types.len()
        && refs.protos.len() == dex.prototypes.len()
        && refs.fields.len() == dex.fields.len()
        && refs.methods.len() == dex.methods.len()
        && refs.call_sites.len() == dex.call_sites.len()
        && refs.method_handles.len() == dex.method_handles.len();

    if nothing_to_compact {
        return;
    }

    let string_remap = build_compact_remap(&refs.strings, dex.strings.len());
    let type_remap = build_compact_remap(&refs.types, dex.types.len());
    let proto_remap = build_compact_remap(&refs.protos, dex.prototypes.len());
    let field_remap = build_compact_remap(&refs.fields, dex.fields.len());
    let method_remap = build_compact_remap(&refs.methods, dex.methods.len());
    let cs_remap = build_compact_remap(&refs.call_sites, dex.call_sites.len());
    let mh_remap = build_compact_remap(&refs.method_handles, dex.method_handles.len());

    dex.strings = filter_indexed(&dex.strings, &refs.strings);
    dex.types = filter_indexed(&dex.types, &refs.types);
    dex.prototypes = filter_indexed(&dex.prototypes, &refs.protos);
    dex.fields = filter_indexed(&dex.fields, &refs.fields);
    dex.methods = filter_indexed(&dex.methods, &refs.methods);
    dex.call_sites = filter_indexed(&dex.call_sites, &refs.call_sites);
    dex.method_handles = filter_indexed(&dex.method_handles, &refs.method_handles);

    for t in &mut dex.types {
        t.0 = string_remap[t.0 as usize];
    }
    for p in &mut dex.prototypes {
        p.shorty = crate::types::StringIdx(string_remap[p.shorty.0 as usize]);
        p.return_type = crate::types::TypeIdx(type_remap[p.return_type.0 as usize]);
        for param in &mut p.parameters {
            param.0 = type_remap[param.0 as usize];
        }
    }
    for f in &mut dex.fields {
        f.class = crate::types::TypeIdx(type_remap[f.class.0 as usize]);
        f.type_ = crate::types::TypeIdx(type_remap[f.type_.0 as usize]);
        f.name = crate::types::StringIdx(string_remap[f.name.0 as usize]);
    }
    for m in &mut dex.methods {
        m.class = crate::types::TypeIdx(type_remap[m.class.0 as usize]);
        m.proto = crate::types::ProtoIdx(proto_remap[m.proto.0 as usize] as u16);
        m.name = crate::types::StringIdx(string_remap[m.name.0 as usize]);
    }
    for cs in &mut dex.call_sites {
        cs.bootstrap_method = MethodHandleIdx(mh_remap[cs.bootstrap_method.0 as usize]);
    }

    let remap = Remap {
        string: &string_remap,
        type_: &type_remap,
        proto: &proto_remap,
        field: &field_remap,
        method: &method_remap,
    };

    for class in &mut dex.classes {
        remap.remap_class(class);
    }
    for cs in &mut dex.call_sites {
        remap.remap_call_site(cs);
    }
    for mh in &mut dex.method_handles {
        remap.remap_method_handle(mh);
    }

    remap_all_exotic_indices(dex, &cs_remap, &mh_remap);
    dex.invalidate_lookups();
}
