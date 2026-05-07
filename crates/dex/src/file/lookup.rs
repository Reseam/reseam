// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use super::DexFile;
use crate::types::class::ClassDef;
use crate::types::{
    DexString, FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};

type StringBucket = SmallVec<[StringIdx; 1]>;
type ProtoBucket = SmallVec<[ProtoIdx; 1]>;

#[derive(Debug, Default)]
pub(crate) struct StringLookup {
    inner: OnceLock<FxHashMap<u64, StringBucket>>,
}

impl StringLookup {
    fn build(strings: &[DexString]) -> FxHashMap<u64, StringBucket> {
        let mut map: FxHashMap<u64, StringBucket> =
            FxHashMap::with_capacity_and_hasher(strings.len(), Default::default());
        for (i, s) in strings.iter().enumerate() {
            let h = hash_str(s.as_str());
            map.entry(h).or_default().push(StringIdx(i as u32));
        }
        map
    }

    fn find(&self, s: &str, strings: &[DexString]) -> Option<StringIdx> {
        let map = self.inner.get_or_init(|| Self::build(strings));
        let h = hash_str(s);
        map.get(&h)?
            .iter()
            .copied()
            .find(|idx| strings[idx.0 as usize].as_str() == s)
    }

    fn insert_after_init(&mut self, s: &str, idx: StringIdx) {
        if let Some(map) = self.inner.get_mut() {
            let h = hash_str(s);
            map.entry(h).or_default().push(idx);
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.inner.take();
    }

    fn ensure(&mut self, strings: &[DexString]) {
        if self.inner.get().is_none() {
            let _ = self.inner.set(Self::build(strings));
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ProtoLookup {
    inner: OnceLock<FxHashMap<u64, ProtoBucket>>,
}

impl ProtoLookup {
    fn build(prototypes: &[Prototype]) -> FxHashMap<u64, ProtoBucket> {
        let mut map: FxHashMap<u64, ProtoBucket> =
            FxHashMap::with_capacity_and_hasher(prototypes.len(), Default::default());
        for (i, p) in prototypes.iter().enumerate() {
            let h = hash_proto(p.return_type, &p.parameters);
            map.entry(h).or_default().push(ProtoIdx(i as u16));
        }
        map
    }

    fn find(
        &self,
        ret: TypeIdx,
        params: &[TypeIdx],
        prototypes: &[Prototype],
    ) -> Option<ProtoIdx> {
        let map = self.inner.get_or_init(|| Self::build(prototypes));
        let h = hash_proto(ret, params);
        map.get(&h)?.iter().copied().find(|idx| {
            let p = &prototypes[idx.0 as usize];
            p.return_type == ret && p.parameters.as_slice() == params
        })
    }

    fn insert_after_init(&mut self, ret: TypeIdx, params: &[TypeIdx], idx: ProtoIdx) {
        if let Some(map) = self.inner.get_mut() {
            let h = hash_proto(ret, params);
            map.entry(h).or_default().push(idx);
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.inner.take();
    }

    fn ensure(&mut self, prototypes: &[Prototype]) {
        if self.inner.get().is_none() {
            let _ = self.inner.set(Self::build(prototypes));
        }
    }
}

#[derive(Debug)]
pub(crate) struct LazyMap<K, V> {
    inner: OnceLock<FxHashMap<K, V>>,
}

impl<K, V> Default for LazyMap<K, V> {
    fn default() -> Self {
        Self {
            inner: OnceLock::new(),
        }
    }
}

impl<K: Hash + Eq, V> LazyMap<K, V> {
    fn get_or_build<F>(&self, build: F) -> &FxHashMap<K, V>
    where
        F: FnOnce() -> FxHashMap<K, V>,
    {
        self.inner.get_or_init(build)
    }

    fn ensure<F>(&mut self, build: F) -> &mut FxHashMap<K, V>
    where
        F: FnOnce() -> FxHashMap<K, V>,
    {
        if self.inner.get().is_none() {
            let _ = self.inner.set(build());
        }
        self.inner.get_mut().unwrap()
    }

    fn get_mut(&mut self) -> Option<&mut FxHashMap<K, V>> {
        self.inner.get_mut()
    }

    pub(crate) fn invalidate(&mut self) {
        self.inner.take();
    }
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    hasher.finish()
}

fn hash_proto(ret: TypeIdx, params: &[TypeIdx]) -> u64 {
    let mut hasher = FxHasher::default();
    ret.hash(&mut hasher);
    for p in params {
        p.hash(&mut hasher);
    }
    hasher.finish()
}

fn build_type_map(types: &[StringIdx]) -> FxHashMap<StringIdx, TypeIdx> {
    let mut map = FxHashMap::with_capacity_and_hasher(types.len(), Default::default());
    for (i, &desc) in types.iter().enumerate() {
        map.insert(desc, TypeIdx(i as u32));
    }
    map
}

fn build_class_map(classes: &[ClassDef]) -> FxHashMap<TypeIdx, usize> {
    let mut map = FxHashMap::with_capacity_and_hasher(classes.len(), Default::default());
    for (i, class) in classes.iter().enumerate() {
        map.insert(class.class_type, i);
    }
    map
}

fn build_method_map(
    methods: &[MethodId],
) -> FxHashMap<(TypeIdx, StringIdx, ProtoIdx), MethodIdx> {
    let mut map = FxHashMap::with_capacity_and_hasher(methods.len(), Default::default());
    for (i, m) in methods.iter().enumerate() {
        map.insert((m.class, m.name, m.proto), MethodIdx(i as u32));
    }
    map
}

fn build_field_map(
    fields: &[FieldId],
) -> FxHashMap<(TypeIdx, StringIdx, TypeIdx), FieldIdx> {
    let mut map = FxHashMap::with_capacity_and_hasher(fields.len(), Default::default());
    for (i, f) in fields.iter().enumerate() {
        map.insert((f.class, f.name, f.type_), FieldIdx(i as u32));
    }
    map
}

impl DexFile {
    pub fn string(&self, idx: StringIdx) -> &str {
        self.strings[idx.0 as usize].as_str()
    }

    pub fn type_descriptor(&self, idx: TypeIdx) -> &str {
        let string_idx = self.types[idx.0 as usize];
        self.string(string_idx)
    }

    pub fn classes(&self) -> &[ClassDef] {
        &self.classes
    }

    pub fn find_class(&self, descriptor: &str) -> Option<&ClassDef> {
        let type_idx = self.find_type_idx(descriptor)?;
        let &idx = self
            .class_lookup
            .get_or_build(|| build_class_map(&self.classes))
            .get(&type_idx)?;
        self.classes.get(idx)
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<&mut ClassDef> {
        let type_idx = self.find_type_idx(descriptor)?;
        let &idx = self
            .class_lookup
            .get_or_build(|| build_class_map(&self.classes))
            .get(&type_idx)?;
        self.classes.get_mut(idx)
    }

    pub fn find_class_index(&self, descriptor: &str) -> Option<usize> {
        let type_idx = self.find_type_idx(descriptor)?;
        self.class_lookup
            .get_or_build(|| build_class_map(&self.classes))
            .get(&type_idx)
            .copied()
    }

    pub fn find_string_idx(&self, s: &str) -> Option<StringIdx> {
        self.string_lookup.find(s, &self.strings)
    }

    pub fn find_type_idx(&self, descriptor: &str) -> Option<TypeIdx> {
        let string_idx = self.find_string_idx(descriptor)?;
        self.type_lookup
            .get_or_build(|| build_type_map(&self.types))
            .get(&string_idx)
            .copied()
    }

    pub(crate) fn find_proto_idx(&self, ret: TypeIdx, params: &[TypeIdx]) -> Option<ProtoIdx> {
        self.proto_lookup.find(ret, params, &self.prototypes)
    }

    pub(crate) fn find_method_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        proto: ProtoIdx,
    ) -> Option<MethodIdx> {
        self.method_lookup
            .get_or_build(|| build_method_map(&self.methods))
            .get(&(class, name, proto))
            .copied()
    }

    pub(crate) fn find_field_idx(
        &self,
        class: TypeIdx,
        name: StringIdx,
        type_: TypeIdx,
    ) -> Option<FieldIdx> {
        self.field_lookup
            .get_or_build(|| build_field_map(&self.fields))
            .get(&(class, name, type_))
            .copied()
    }

    pub(crate) fn class_lookup_get(&self, type_idx: TypeIdx) -> Option<usize> {
        self.class_lookup
            .get_or_build(|| build_class_map(&self.classes))
            .get(&type_idx)
            .copied()
    }

    pub(crate) fn record_string(&mut self, s: &str, idx: StringIdx) {
        self.string_lookup.insert_after_init(s, idx);
    }

    pub(crate) fn record_type(&mut self, string_idx: StringIdx, idx: TypeIdx) {
        if let Some(map) = self.type_lookup.get_mut() {
            map.insert(string_idx, idx);
        }
    }

    pub(crate) fn record_class(&mut self, type_idx: TypeIdx, position: usize) {
        if let Some(map) = self.class_lookup.get_mut() {
            map.insert(type_idx, position);
        }
    }

    pub(crate) fn record_proto(&mut self, ret: TypeIdx, params: &[TypeIdx], idx: ProtoIdx) {
        self.proto_lookup.insert_after_init(ret, params, idx);
    }

    pub(crate) fn record_method(
        &mut self,
        class: TypeIdx,
        name: StringIdx,
        proto: ProtoIdx,
        idx: MethodIdx,
    ) {
        if let Some(map) = self.method_lookup.get_mut() {
            map.insert((class, name, proto), idx);
        }
    }

    pub(crate) fn record_field(
        &mut self,
        class: TypeIdx,
        name: StringIdx,
        type_: TypeIdx,
        idx: FieldIdx,
    ) {
        if let Some(map) = self.field_lookup.get_mut() {
            map.insert((class, name, type_), idx);
        }
    }

    pub fn build_lookups(&mut self) {
        self.invalidate_lookups();
        self.string_lookup.ensure(&self.strings);
        self.proto_lookup.ensure(&self.prototypes);
        self.type_lookup.ensure(|| build_type_map(&self.types));
        self.class_lookup.ensure(|| build_class_map(&self.classes));
        self.method_lookup
            .ensure(|| build_method_map(&self.methods));
        self.field_lookup
            .ensure(|| build_field_map(&self.fields));
    }

    pub fn invalidate_lookups(&mut self) {
        self.string_lookup.invalidate();
        self.proto_lookup.invalidate();
        self.type_lookup.invalidate();
        self.class_lookup.invalidate();
        self.method_lookup.invalidate();
        self.field_lookup.invalidate();
    }
}
