// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything the writer reads goes through a [`WritePlan`]: the output order
//! of every pool, the old-to-new remap that order implies, and the classes in
//! output order. The [`DexFile`] itself is never mutated by a write, so it
//! stays consistent afterwards and can be written again.

use std::borrow::Cow;

use super::sort::{fixup_code, Remap, RemapTables};
use crate::error::Result;
use crate::file::{ClassHeader, DexFile, RawClassDef};
use crate::read::annotation::read_annotations_directory;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::types::annotation::AnnotationsDirectory;
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;
use crate::types::method_handle::{CallSiteItem, MethodHandle};
use crate::types::{FieldId, MethodId, Prototype, StringIdx, TypeIdx, TypeList};

/// Output order of each pool: `order[new] = old`.
pub(crate) struct PoolOrder {
    pub string: Vec<u32>,
    pub type_: Vec<u32>,
    pub proto: Vec<u32>,
    pub field: Vec<u32>,
    pub method: Vec<u32>,
}

/// A class in output order: a patch-touched class from its (remapped) IR, or
/// a class still in the file, remapped as it is read.
pub(crate) enum WriteClass<'a> {
    Resident(Cow<'a, ClassDef>),
    Raw(RawClassDef),
}

pub(crate) struct WritePlan<'a> {
    pub dex: &'a DexFile,
    /// `None` when every pool is already in DEX sort order.
    pub order: Option<PoolOrder>,
    pub remap: Option<RemapTables>,
    pub classes: Vec<WriteClass<'a>>,
}

impl<'a> WritePlan<'a> {
    /// Sorts every pool and resolves what each class needs. A pool already in
    /// sort order whose keys reference only identity-mapped pools costs
    /// nothing; resident classes are cloned and remapped only when a pool
    /// actually moved.
    pub(crate) fn new(dex: &'a DexFile) -> Result<Self> {
        let string_remap = (!dex.strings.is_sorted()).then(|| {
            let mut order: Vec<u32> = (0..dex.strings.len() as u32).collect();
            order.sort_by(|&a, &b| dex.strings.compare(a, b));
            order
        });
        let string_remap = string_remap.map(|order| build_remap(&order));
        let map_string = |i: StringIdx| string_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

        let type_remap = (!dex.types.is_sorted() || string_remap.is_some()).then(|| {
            let mut order: Vec<u32> = (0..dex.types.len() as u32).collect();
            order.sort_by_key(|&i| map_string(dex.types.get(i as usize)));
            build_remap(&order)
        });
        let map_type = |i: TypeIdx| type_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

        let proto_remap = (!dex.prototypes.is_sorted() || type_remap.is_some()).then(|| {
            let mut order: Vec<u32> = (0..dex.prototypes.len() as u32).collect();
            order.sort_by(|&a, &b| {
                let pa = dex.prototypes.get(a as usize);
                let pb = dex.prototypes.get(b as usize);
                map_type(pa.return_type)
                    .cmp(&map_type(pb.return_type))
                    .then_with(|| pa.parameters.iter().map(|t| map_type(*t)).cmp(pb.parameters.iter().map(|t| map_type(*t))))
            });
            build_remap(&order)
        });
        let map_proto = |i: crate::types::ProtoIdx| proto_remap.as_ref().map_or(i.0 as u32, |r| r[i.0 as usize]);

        let field_remap = (!dex.fields.is_sorted() || type_remap.is_some() || string_remap.is_some()).then(|| {
            let mut order: Vec<u32> = (0..dex.fields.len() as u32).collect();
            order.sort_by_cached_key(|&i| {
                let f = dex.fields.get(i as usize);
                (map_type(f.class), map_string(f.name), map_type(f.type_))
            });
            build_remap(&order)
        });

        let method_remap = (!dex.methods.is_sorted() || type_remap.is_some() || string_remap.is_some() || proto_remap.is_some()).then(|| {
            let mut order: Vec<u32> = (0..dex.methods.len() as u32).collect();
            order.sort_by_cached_key(|&i| {
                let m = dex.methods.get(i as usize);
                (map_type(m.class), map_string(m.name), map_proto(m.proto))
            });
            build_remap(&order)
        });

        let remaps = [
            string_remap.as_deref(),
            type_remap.as_deref(),
            proto_remap.as_deref(),
            field_remap.as_deref(),
            method_remap.as_deref(),
        ];
        let already_sorted = remaps.iter().all(|r| r.is_none_or(is_identity));

        let (order, remap) = if already_sorted {
            (None, None)
        } else {
            let identity = |len: usize| (0..len as u32).collect::<Vec<u32>>();
            let remap = RemapTables {
                string: string_remap.unwrap_or_else(|| identity(dex.strings.len())),
                type_: type_remap.unwrap_or_else(|| identity(dex.types.len())),
                proto: proto_remap.unwrap_or_else(|| identity(dex.prototypes.len())),
                field: field_remap.unwrap_or_else(|| identity(dex.fields.len())),
                method: method_remap.unwrap_or_else(|| identity(dex.methods.len())),
            };
            let order = PoolOrder {
                string: invert(&remap.string),
                type_: invert(&remap.type_),
                proto: invert(&remap.proto),
                field: invert(&remap.field),
                method: invert(&remap.method),
            };
            (Some(order), Some(remap))
        };

        let mut classes: Vec<WriteClass<'a>> = Vec::with_capacity(dex.classes.len());
        for i in 0..dex.classes.len() {
            classes.push(match (dex.classes.resident(i), dex.classes.raw_def(i)) {
                (Some(class), _) => match &remap {
                    Some(remap) => {
                        let mut class = class.clone();
                        remap.as_remap().remap_class(&mut class);
                        fixup_class(&mut class)?;
                        WriteClass::Resident(Cow::Owned(class))
                    }
                    None => WriteClass::Resident(Cow::Borrowed(class)),
                },
                (None, Some(raw)) => WriteClass::Raw(raw),
                (None, None) => unreachable!("every slot is resident or raw"),
            });
        }
        let type_of = |class: &WriteClass<'_>| match class {
            WriteClass::Resident(c) => c.class_type.0,
            WriteClass::Raw(raw) => remap
                .as_ref()
                .map_or(raw.class_type, |r| r.type_[raw.class_type as usize]),
        };
        classes.sort_by_key(type_of);

        Ok(Self {
            dex,
            order,
            remap,
            classes,
        })
    }

    pub(crate) fn remap(&self) -> Option<Remap<'_>> {
        self.remap.as_ref().map(RemapTables::as_remap)
    }

    pub(crate) fn string_count(&self) -> usize {
        self.dex.strings.len()
    }

    pub(crate) fn string_item(&self, new: usize) -> Cow<'_, [u8]> {
        let old = self.order.as_ref().map_or(new as u32, |o| o.string[new]);
        self.dex.strings.item(StringIdx(old))
    }

    pub(crate) fn types(&self) -> impl Iterator<Item = StringIdx> + '_ {
        (0..self.dex.types.len()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.type_[new]);
            self.map_string(self.dex.types.get(old as usize))
        })
    }

    pub(crate) fn prototypes(&self) -> impl Iterator<Item = Prototype> + '_ {
        (0..self.dex.prototypes.len()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.proto[new]);
            let p = self.dex.prototypes.get(old as usize);
            Prototype {
                shorty: self.map_string(p.shorty),
                return_type: self.map_type(p.return_type),
                parameters: p.parameters.iter().map(|t| self.map_type(*t)).collect(),
            }
        })
    }

    pub(crate) fn fields(&self) -> impl Iterator<Item = FieldId> + '_ {
        (0..self.dex.fields.len()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.field[new]);
            let f = self.dex.fields.get(old as usize);
            FieldId {
                class: self.map_type(f.class),
                type_: self.map_type(f.type_),
                name: self.map_string(f.name),
            }
        })
    }

    pub(crate) fn methods(&self) -> impl Iterator<Item = MethodId> + '_ {
        (0..self.dex.methods.len()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.method[new]);
            let m = self.dex.methods.get(old as usize);
            MethodId {
                class: self.map_type(m.class),
                proto: self.map_proto(m.proto),
                name: self.map_string(m.name),
            }
        })
    }

    pub(crate) fn call_sites(&self) -> Cow<'_, [CallSiteItem]> {
        match self.remap() {
            None => Cow::Borrowed(&self.dex.call_sites),
            Some(remap) => {
                let mut sites = self.dex.call_sites.clone();
                sites.iter_mut().for_each(|cs| remap.remap_call_site(cs));
                Cow::Owned(sites)
            }
        }
    }

    pub(crate) fn method_handles(&self) -> Cow<'_, [MethodHandle]> {
        match self.remap() {
            None => Cow::Borrowed(&self.dex.method_handles),
            Some(remap) => {
                let mut handles = self.dex.method_handles.clone();
                handles.iter_mut().for_each(|mh| remap.remap_method_handle(mh));
                Cow::Owned(handles)
            }
        }
    }

    pub(crate) fn class_header(&self, k: usize) -> ClassHeader {
        match &self.classes[k] {
            WriteClass::Resident(c) => ClassHeader::of(c),
            WriteClass::Raw(raw) => {
                let h = raw.header();
                ClassHeader {
                    class_type: self.map_type(h.class_type),
                    access_flags: h.access_flags,
                    superclass: h.superclass.map(|t| self.map_type(t)),
                    source_file: h.source_file.map(|s| self.map_string(s)),
                }
            }
        }
    }

    pub(crate) fn class_interfaces(&self, k: usize) -> TypeList {
        match &self.classes[k] {
            WriteClass::Resident(c) => c.interfaces.clone(),
            WriteClass::Raw(raw) => {
                if raw.interfaces_off == 0 {
                    return TypeList::new();
                }
                crate::file::read_type_list(self.raw_bytes(), raw.interfaces_off as usize)
                    .iter()
                    .map(|t| self.map_type(*t))
                    .collect()
            }
        }
    }

    pub(crate) fn class_annotations(&self, k: usize) -> Result<Option<Cow<'_, AnnotationsDirectory>>> {
        match &self.classes[k] {
            WriteClass::Resident(c) => Ok(c.annotations.as_deref().map(Cow::Borrowed)),
            WriteClass::Raw(raw) => {
                if raw.annotations_off == 0 || !self.dex.parse_options.include_annotations {
                    return Ok(None);
                }
                let mut dir =
                    read_annotations_directory(self.raw_bytes(), raw.annotations_off, &self.dex.parse_options)?;
                if let Some(remap) = self.remap() {
                    remap.remap_annotations_dir(&mut dir);
                }
                Ok(Some(Cow::Owned(dir)))
            }
        }
    }

    pub(crate) fn class_static_values(&self, k: usize) -> Result<Cow<'_, [EncodedValue]>> {
        match &self.classes[k] {
            WriteClass::Resident(c) => Ok(Cow::Borrowed(&c.static_values)),
            WriteClass::Raw(raw) => {
                if raw.static_values_off == 0 {
                    return Ok(Cow::Borrowed(&[]));
                }
                let (mut values, _) = read_encoded_array_with_opts(
                    self.raw_bytes(),
                    raw.static_values_off as usize,
                    &self.dex.parse_options,
                )?;
                if let Some(remap) = self.remap() {
                    values.iter_mut().for_each(|v| remap.remap_encoded_value(v));
                }
                Ok(Cow::Owned(values))
            }
        }
    }

    pub(crate) fn raw_bytes(&self) -> &'a [u8] {
        self.dex
            .raw
            .as_ref()
            .expect("file classes exist only while the buffer is retained")
            .as_bytes()
    }

    fn map_string(&self, idx: StringIdx) -> StringIdx {
        match &self.remap {
            Some(r) => StringIdx(r.string[idx.0 as usize]),
            None => idx,
        }
    }

    fn map_type(&self, idx: TypeIdx) -> TypeIdx {
        match &self.remap {
            Some(r) => TypeIdx(r.type_[idx.0 as usize]),
            None => idx,
        }
    }

    fn map_proto(&self, idx: crate::types::ProtoIdx) -> crate::types::ProtoIdx {
        match &self.remap {
            Some(r) => crate::types::ProtoIdx(r.proto[idx.0 as usize] as u16),
            None => idx,
        }
    }
}

fn fixup_class(class: &mut ClassDef) -> Result<()> {
    let Some(data) = class.class_data.as_mut() else {
        return Ok(());
    };
    for method in data.direct_methods.iter_mut().chain(data.virtual_methods.iter_mut()) {
        if let Some(code) = method.code.as_mut() {
            fixup_code(code)?;
        }
    }
    Ok(())
}

/// Build old_idx → new_idx remap from a permutation array (new_idx → old_idx).
fn build_remap(order: &[u32]) -> Vec<u32> {
    invert(order)
}

/// Inverts a permutation: `out[perm[i]] = i`.
fn invert(perm: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; perm.len()];
    for (i, &p) in perm.iter().enumerate() {
        out[p as usize] = i as u32;
    }
    out
}

fn is_identity(remap: &[u32]) -> bool {
    remap.iter().enumerate().all(|(i, &v)| v == i as u32)
}
