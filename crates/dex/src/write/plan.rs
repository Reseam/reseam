// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod class;

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rustc_hash::FxHashMap;

use super::part::DexPart;
use super::sort::{Remap, RemapTables};
use crate::error::{Result, invalid};
use crate::file::{ClassHeader, DexFile, RawClassDef};
use crate::read::annotation::read_annotations_directory;
use crate::references::pool_len;
use crate::types::annotation::AnnotationsDirectory;
use crate::types::class::ClassDef;
use crate::types::encoded_value::EncodedValue;
use crate::types::method_handle::{CallSiteItem, MethodHandle};
use crate::types::{FieldId, MethodId, Pool, Prototype, StringIdx, TypeIdx, TypeList};

pub(crate) struct PoolOrder {
    pub string: Vec<u32>,
    pub type_: Vec<u32>,
    pub proto: Vec<u32>,
    pub field: Vec<u32>,
    pub method: Vec<u32>,
    pub call_site: Vec<u32>,
    pub method_handle: Vec<u32>,
}

pub(crate) enum WriteClass<'a> {
    Resident(&'a ClassDef),
    Raw(RawClassDef),
}

pub(crate) struct WritePlan<'a> {
    pub dex: &'a DexFile,
    pub options: super::WriteOptions,
    /// `None` when every pool is written whole and already in DEX sort order.
    pub order: Option<PoolOrder>,
    pub remap: Option<RemapTables>,
    pub classes: Vec<WriteClass<'a>>,
    /// Source class-table index for each output class, including class-indexed metadata.
    pub class_order: Vec<usize>,
}

impl<'a> WritePlan<'a> {
    pub(crate) fn new(
        dex: &'a DexFile,
        part: Option<&DexPart>,
        options: super::WriteOptions,
    ) -> Result<Self> {
        crate::references::validate_pools(dex)?;
        if let Some(part) = part {
            crate::references::validate_part(dex, part)?;
        }
        let compact = part.is_some();
        let candidates = |pool| pool_candidates(dex, part, pool);

        let strings = ordered_strings(dex, part);
        let string_remap = strings.as_deref().map(|o| remap_of(o, dex.strings.len()));
        let map_string = |i: StringIdx| string_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

        let types = (compact || !dex.types.is_sorted() || string_remap.is_some()).then(|| {
            let mut order = candidates(Pool::Type);
            order.sort_by_key(|&i| map_string(dex.types.get(i as usize)));
            order
        });
        let type_remap = types.as_deref().map(|o| remap_of(o, dex.types.len()));
        let map_type = |i: TypeIdx| type_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

        let protos = (compact || !dex.prototypes.is_sorted() || type_remap.is_some()).then(|| {
            let mut order = candidates(Pool::Proto);
            order.sort_by(|&a, &b| {
                let pa = dex.prototypes.get(a as usize);
                let pb = dex.prototypes.get(b as usize);
                map_type(pa.return_type)
                    .cmp(&map_type(pb.return_type))
                    .then_with(|| {
                        pa.parameters
                            .iter()
                            .map(|t| map_type(*t))
                            .cmp(pb.parameters.iter().map(|t| map_type(*t)))
                    })
            });
            order
        });
        let proto_remap = protos.as_deref().map(|o| remap_of(o, dex.prototypes.len()));
        let map_proto =
            |i: crate::types::ProtoIdx| proto_remap.as_ref().map_or(i.0, |r| r[i.0 as usize]);

        let fields =
            (compact || !dex.fields.is_sorted() || type_remap.is_some() || string_remap.is_some())
                .then(|| {
                    ordered_pool(dex, part, Pool::Field, |i| {
                        let f = dex.fields.get(i as usize);
                        FieldId {
                            class: TypeIdx(map_type(f.class)),
                            name: StringIdx(map_string(f.name)),
                            type_: TypeIdx(map_type(f.type_)),
                        }
                    })
                });
        let field_remap = fields.as_deref().map(|o| remap_of(o, dex.fields.len()));

        let methods = (compact
            || !dex.methods.is_sorted()
            || type_remap.is_some()
            || string_remap.is_some()
            || proto_remap.is_some())
        .then(|| {
            ordered_pool(dex, part, Pool::Method, |i| {
                let m = dex.methods.get(i as usize);
                MethodId {
                    class: TypeIdx(map_type(m.class)),
                    name: StringIdx(map_string(m.name)),
                    proto: crate::ProtoIdx(map_proto(m.proto)),
                }
            })
        });
        let method_remap = methods.as_deref().map(|o| remap_of(o, dex.methods.len()));

        let already_sorted = !compact
            && unchanged_pools([
                &string_remap,
                &type_remap,
                &proto_remap,
                &field_remap,
                &method_remap,
            ]);

        let (order, remap) = if already_sorted {
            (None, None)
        } else {
            let order = PoolOrder {
                string: strings.unwrap_or_else(|| candidates(Pool::String)),
                type_: types.unwrap_or_else(|| candidates(Pool::Type)),
                proto: protos.unwrap_or_else(|| candidates(Pool::Proto)),
                field: fields.unwrap_or_else(|| candidates(Pool::Field)),
                method: methods.unwrap_or_else(|| candidates(Pool::Method)),
                call_site: candidates(Pool::CallSite),
                method_handle: candidates(Pool::MethodHandle),
            };
            let remap = pool_remap(dex, &order);
            (Some(order), Some(remap))
        };

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

    pub(crate) fn remap(&self) -> Option<Remap<'_>> {
        self.remap.as_ref().map(RemapTables::as_remap)
    }

    pub(crate) fn string_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.strings.len(), |o| o.string.len())
    }

    pub(crate) fn type_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.types.len(), |o| o.type_.len())
    }

    pub(crate) fn proto_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.prototypes.len(), |o| o.proto.len())
    }

    pub(crate) fn field_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.fields.len(), |o| o.field.len())
    }

    pub(crate) fn method_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.methods.len(), |o| o.method.len())
    }

    pub(crate) fn call_site_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.call_sites.len(), |o| o.call_site.len())
    }

    pub(crate) fn method_handle_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.method_handles.len(), |o| o.method_handle.len())
    }

    pub(crate) fn string_item(&self, new: usize) -> Cow<'_, [u8]> {
        let old = self.order.as_ref().map_or(new as u32, |o| o.string[new]);
        self.dex.strings.item(StringIdx(old))
    }

    pub(crate) fn types(&self) -> impl Iterator<Item = StringIdx> + '_ {
        (0..self.type_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.type_[new]);
            self.map_string(self.dex.types.get(old as usize))
        })
    }

    pub(crate) fn prototypes(&self) -> impl Iterator<Item = Prototype> + '_ {
        (0..self.proto_count()).map(move |new| {
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
        (0..self.field_count()).map(move |new| {
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
        (0..self.method_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.method[new]);
            let m = self.dex.methods.get(old as usize);
            MethodId {
                class: self.map_type(m.class),
                proto: self.map_proto(m.proto),
                name: self.map_string(m.name),
            }
        })
    }

    pub(crate) fn call_sites(&self) -> impl Iterator<Item = Result<Cow<'_, CallSiteItem>>> {
        (0..self.call_site_count()).map(|new| {
            let old = self
                .order
                .as_ref()
                .map_or(new as u32, |order| order.call_site[new]);
            let mut site = self.dex.call_sites.get(old as usize)?;
            if let Some(remap) = self.remap() {
                remap.remap_call_site(site.to_mut())?;
            }
            Ok(site)
        })
    }
    pub(crate) fn method_handles(&self) -> impl Iterator<Item = Result<MethodHandle>> + '_ {
        (0..self.method_handle_count()).map(|new| {
            let old = self
                .order
                .as_ref()
                .map_or(new as u32, |order| order.method_handle[new]);
            let mut handle = *self.dex.method_handles.get(old as usize)?;
            if let Some(remap) = self.remap() {
                remap.remap_method_handle(&mut handle)?;
            }
            Ok(handle)
        })
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
            Some(r) => crate::types::ProtoIdx(r.proto[idx.0 as usize]),
            None => idx,
        }
    }
}

fn remap_of(order: &[u32], len: usize) -> Vec<u32> {
    let mut remap = vec![u32::MAX; len];
    for (new, &old) in order.iter().enumerate() {
        remap[old as usize] = new as u32;
    }
    remap
}

fn is_identity(remap: &[u32]) -> bool {
    remap.iter().enumerate().all(|(i, &v)| v == i as u32)
}

fn pool_candidates(dex: &DexFile, part: Option<&DexPart>, pool: Pool) -> Vec<u32> {
    match part {
        Some(part) => part.pools.members(pool).collect(),
        None => (0..pool_len(dex, pool) as u32).collect(),
    }
}

fn pool_remap(dex: &DexFile, order: &PoolOrder) -> RemapTables {
    RemapTables {
        string: remap_of(&order.string, dex.strings.len()),
        type_: remap_of(&order.type_, dex.types.len()),
        proto: remap_of(&order.proto, dex.prototypes.len()),
        field: remap_of(&order.field, dex.fields.len()),
        method: remap_of(&order.method, dex.methods.len()),
        call_site: remap_of(&order.call_site, dex.call_sites.len()),
        method_handle: remap_of(&order.method_handle, dex.method_handles.len()),
    }
}

fn unchanged_pools(maps: [&Option<Vec<u32>>; 5]) -> bool {
    maps.into_iter()
        .all(|map| map.as_deref().is_none_or(is_identity))
}

fn ordered_strings(dex: &DexFile, part: Option<&DexPart>) -> Option<Vec<u32>> {
    (part.is_some() || !dex.strings.is_sorted()).then(|| {
        let mut order = pool_candidates(dex, part, Pool::String);
        order.sort_by(|&a, &b| dex.strings.compare(a, b));
        order
    })
}

fn ordered_pool<K: Ord>(
    dex: &DexFile,
    part: Option<&DexPart>,
    pool: Pool,
    mut key: impl FnMut(u32) -> K,
) -> Vec<u32> {
    let mut order = pool_candidates(dex, part, pool);
    order.sort_by_cached_key(|&index| key(index));
    order
}
