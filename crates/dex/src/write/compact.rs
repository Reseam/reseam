use std::collections::{HashMap, HashSet};

use crate::file::DexFile;
use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::{ClassData, ClassDef};
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::instruction::Instruction;
use crate::types::method_handle::{
    CallSiteIdx, CallSiteItem, MethodHandle, MethodHandleIdx, MethodHandleMember,
};
use crate::types::{ProtoIdx, StringIdx, TypeIdx};

use super::sort::Remap;
use super::MAX_POOL_SIZE;

struct ReferencedIndices {
    strings: HashSet<u32>,
    types: HashSet<u32>,
    protos: HashSet<u32>,
    fields: HashSet<u32>,
    methods: HashSet<u32>,
    call_sites: HashSet<u32>,
    method_handles: HashSet<u32>,
}

impl ReferencedIndices {
    fn new() -> Self {
        Self {
            strings: HashSet::new(),
            types: HashSet::new(),
            protos: HashSet::new(),
            fields: HashSet::new(),
            methods: HashSet::new(),
            call_sites: HashSet::new(),
            method_handles: HashSet::new(),
        }
    }

    fn collect_from_dex(dex: &DexFile) -> Self {
        let mut refs = Self::new();
        for class in &dex.classes {
            refs.collect_class(class);
        }
        for cs in &dex.call_sites {
            refs.collect_call_site(cs);
        }
        for mh in &dex.method_handles {
            refs.collect_method_handle(mh);
        }
        refs.expand_transitive(dex);
        refs
    }

    fn collect_from_classes(classes: &[ClassDef]) -> Self {
        let mut refs = Self::new();
        for class in classes {
            refs.collect_class(class);
        }
        refs
    }

    fn collect_class(&mut self, class: &ClassDef) {
        self.types.insert(class.class_type.0);
        if let Some(sc) = class.superclass {
            self.types.insert(sc.0);
        }
        for iface in &class.interfaces {
            self.types.insert(iface.0);
        }
        if let Some(sf) = class.source_file {
            self.strings.insert(sf.0);
        }
        if let Some(ann) = &class.annotations {
            self.collect_annotations_dir(ann);
        }
        if let Some(data) = &class.class_data {
            self.collect_class_data(data);
        }
        for sv in &class.static_values {
            self.collect_encoded_value(sv);
        }
    }

    fn collect_class_data(&mut self, data: &ClassData) {
        for f in data.static_fields.iter().chain(&data.instance_fields) {
            self.fields.insert(f.field.0);
        }
        for m in data.direct_methods.iter().chain(&data.virtual_methods) {
            self.methods.insert(m.method.0);
            if let Some(code) = &m.code {
                self.collect_code(code);
            }
        }
    }

    fn collect_code(&mut self, code: &CodeItem) {
        for insn in &code.instructions {
            self.collect_instruction(insn);
        }
        for handler in &code.catch_handlers {
            for tc in &handler.typed_catches {
                self.types.insert(tc.exception_type.0);
            }
        }
        if let Some(debug) = &code.debug_info {
            self.collect_debug(debug);
        }
    }

    fn collect_instruction(&mut self, insn: &Instruction) {
        match insn {
            Instruction::ConstString { string, .. }
            | Instruction::ConstStringJumbo { string, .. } => {
                self.strings.insert(string.0);
            }
            Instruction::ConstClass { type_, .. }
            | Instruction::CheckCast { type_, .. }
            | Instruction::NewInstance { type_, .. }
            | Instruction::InstanceOf { type_, .. }
            | Instruction::NewArray { type_, .. }
            | Instruction::FilledNewArray { type_, .. }
            | Instruction::FilledNewArrayRange { type_, .. } => {
                self.types.insert(type_.0);
            }
            Instruction::Iget { field, .. }
            | Instruction::IgetWide { field, .. }
            | Instruction::IgetObject { field, .. }
            | Instruction::IgetBoolean { field, .. }
            | Instruction::IgetByte { field, .. }
            | Instruction::IgetChar { field, .. }
            | Instruction::IgetShort { field, .. }
            | Instruction::Iput { field, .. }
            | Instruction::IputWide { field, .. }
            | Instruction::IputObject { field, .. }
            | Instruction::IputBoolean { field, .. }
            | Instruction::IputByte { field, .. }
            | Instruction::IputChar { field, .. }
            | Instruction::IputShort { field, .. }
            | Instruction::Sget { field, .. }
            | Instruction::SgetWide { field, .. }
            | Instruction::SgetObject { field, .. }
            | Instruction::SgetBoolean { field, .. }
            | Instruction::SgetByte { field, .. }
            | Instruction::SgetChar { field, .. }
            | Instruction::SgetShort { field, .. }
            | Instruction::Sput { field, .. }
            | Instruction::SputWide { field, .. }
            | Instruction::SputObject { field, .. }
            | Instruction::SputBoolean { field, .. }
            | Instruction::SputByte { field, .. }
            | Instruction::SputChar { field, .. }
            | Instruction::SputShort { field, .. } => {
                self.fields.insert(field.0);
            }
            Instruction::InvokeVirtual { method, .. }
            | Instruction::InvokeSuper { method, .. }
            | Instruction::InvokeDirect { method, .. }
            | Instruction::InvokeStatic { method, .. }
            | Instruction::InvokeInterface { method, .. }
            | Instruction::InvokeVirtualRange { method, .. }
            | Instruction::InvokeSuperRange { method, .. }
            | Instruction::InvokeDirectRange { method, .. }
            | Instruction::InvokeStaticRange { method, .. }
            | Instruction::InvokeInterfaceRange { method, .. } => {
                self.methods.insert(method.0);
            }
            Instruction::InvokePolymorphic { method, proto, .. }
            | Instruction::InvokePolymorphicRange { method, proto, .. } => {
                self.methods.insert(method.0);
                self.protos.insert(proto.0 as u32);
            }
            Instruction::ConstMethodType { proto, .. } => {
                self.protos.insert(proto.0 as u32);
            }
            Instruction::InvokeCustom { call_site, .. }
            | Instruction::InvokeCustomRange { call_site, .. } => {
                self.call_sites.insert(call_site.0);
            }
            Instruction::ConstMethodHandle {
                method_handle, ..
            } => {
                self.method_handles.insert(method_handle.0);
            }
            _ => {}
        }
    }

    fn collect_debug(&mut self, debug: &DebugInfo) {
        for name in &debug.parameter_names {
            if let Some(n) = name {
                self.strings.insert(n.0);
            }
        }
        for bc in &debug.bytecodes {
            match bc {
                DebugBytecode::StartLocal { name, type_, .. } => {
                    if let Some(n) = name {
                        self.strings.insert(n.0);
                    }
                    if let Some(t) = type_ {
                        self.types.insert(t.0);
                    }
                }
                DebugBytecode::StartLocalExtended {
                    name,
                    type_,
                    signature,
                    ..
                } => {
                    if let Some(n) = name {
                        self.strings.insert(n.0);
                    }
                    if let Some(t) = type_ {
                        self.types.insert(t.0);
                    }
                    if let Some(s) = signature {
                        self.strings.insert(s.0);
                    }
                }
                DebugBytecode::SetFile { name } => {
                    if let Some(n) = name {
                        self.strings.insert(n.0);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_annotations_dir(&mut self, dir: &AnnotationsDirectory) {
        for item in &dir.class_annotations {
            self.collect_annotation_item(item);
        }
        for (field_idx, items) in &dir.field_annotations {
            self.fields.insert(field_idx.0);
            for item in items {
                self.collect_annotation_item(item);
            }
        }
        for (method_idx, items) in &dir.method_annotations {
            self.methods.insert(method_idx.0);
            for item in items {
                self.collect_annotation_item(item);
            }
        }
        for (method_idx, param_items) in &dir.parameter_annotations {
            self.methods.insert(method_idx.0);
            for items in param_items {
                for item in items {
                    self.collect_annotation_item(item);
                }
            }
        }
    }

    fn collect_annotation_item(&mut self, item: &AnnotationItem) {
        self.types.insert(item.type_.0);
        for elem in &item.elements {
            self.strings.insert(elem.name.0);
            self.collect_encoded_value(&elem.value);
        }
    }

    fn collect_encoded_value(&mut self, v: &EncodedValue) {
        match v {
            EncodedValue::String(idx) => {
                self.strings.insert(idx.0);
            }
            EncodedValue::Type(idx) => {
                self.types.insert(idx.0);
            }
            EncodedValue::Field(idx) | EncodedValue::Enum(idx) => {
                self.fields.insert(idx.0);
            }
            EncodedValue::Method(idx) => {
                self.methods.insert(idx.0);
            }
            EncodedValue::MethodType(idx) => {
                self.protos.insert(idx.0 as u32);
            }
            EncodedValue::MethodHandle(idx) => {
                self.method_handles.insert(idx.0);
            }
            EncodedValue::Array(items) => {
                for item in items {
                    self.collect_encoded_value(item);
                }
            }
            EncodedValue::Annotation(ann) => {
                self.types.insert(ann.type_.0);
                for elem in &ann.elements {
                    self.strings.insert(elem.name.0);
                    self.collect_encoded_value(&elem.value);
                }
            }
            _ => {}
        }
    }

    fn collect_call_site(&mut self, cs: &CallSiteItem) {
        self.strings.insert(cs.method_name.0);
        self.protos.insert(cs.method_type.0 as u32);
        self.method_handles.insert(cs.bootstrap_method.0);
        for arg in &cs.extra_arguments {
            self.collect_encoded_value(arg);
        }
    }

    fn collect_method_handle(&mut self, mh: &MethodHandle) {
        match &mh.member {
            MethodHandleMember::Field(idx) => {
                self.fields.insert(idx.0);
            }
            MethodHandleMember::Method(idx) => {
                self.methods.insert(idx.0);
            }
        }
    }

    fn expand_transitive(&mut self, dex: &DexFile) {
        // call_sites → bootstrap method_handles
        for cs_idx in self.call_sites.iter().copied().collect::<Vec<_>>() {
            if let Some(cs) = dex.call_sites.get(cs_idx as usize) {
                self.method_handles.insert(cs.bootstrap_method.0);
                self.strings.insert(cs.method_name.0);
                self.protos.insert(cs.method_type.0 as u32);
                for arg in &cs.extra_arguments {
                    self.collect_encoded_value(arg);
                }
            }
        }

        // method_handles → fields/methods
        for mh_idx in self.method_handles.iter().copied().collect::<Vec<_>>() {
            if let Some(mh) = dex.method_handles.get(mh_idx as usize) {
                match &mh.member {
                    MethodHandleMember::Field(idx) => {
                        self.fields.insert(idx.0);
                    }
                    MethodHandleMember::Method(idx) => {
                        self.methods.insert(idx.0);
                    }
                }
            }
        }

        // methods → class type, name string, proto
        for mi in self.methods.iter().copied().collect::<Vec<_>>() {
            let m = &dex.methods[mi as usize];
            self.types.insert(m.class.0);
            self.strings.insert(m.name.0);
            self.protos.insert(m.proto.0 as u32);
        }

        // fields → class type, type, name string
        for fi in self.fields.iter().copied().collect::<Vec<_>>() {
            let f = &dex.fields[fi as usize];
            self.types.insert(f.class.0);
            self.types.insert(f.type_.0);
            self.strings.insert(f.name.0);
        }

        // protos → shorty string, return type, parameter types
        for pi in self.protos.iter().copied().collect::<Vec<_>>() {
            let p = &dex.prototypes[pi as usize];
            self.strings.insert(p.shorty.0);
            self.types.insert(p.return_type.0);
            for param in &p.parameters {
                self.types.insert(param.0);
            }
        }

        // types → descriptor strings
        for ti in self.types.iter().copied().collect::<Vec<_>>() {
            self.strings.insert(dex.types[ti as usize].0);
        }
    }
}

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

    // Build remaps by interning referenced entries into dest
    let string_remap = build_string_remap(&refs.strings, source, dest);
    let type_remap = build_type_remap(&refs.types, source, dest);
    let proto_remap_u32 = build_proto_remap(&refs.protos, source, dest)?;
    let proto_remap: Vec<u16> = proto_remap_u32
        .iter()
        .map(|&v| if v == u32::MAX { u16::MAX } else { v as u16 })
        .collect();
    let method_remap = build_method_remap(&refs.methods, source, dest)?;
    let field_remap = build_field_remap(&refs.fields, source, dest)?;

    // Transplant method_handles (dedup against existing entries in dest)
    let mut mh_remap: HashMap<u32, u32> = HashMap::new();
    for &mh_idx in &refs.method_handles {
        if let Some(handle) = source.method_handles.get(mh_idx as usize) {
            let new_member = match &handle.member {
                MethodHandleMember::Field(idx) => {
                    MethodHandleMember::Field(crate::types::FieldIdx(field_remap[idx.0 as usize]))
                }
                MethodHandleMember::Method(idx) => {
                    MethodHandleMember::Method(crate::types::MethodIdx(
                        method_remap[idx.0 as usize],
                    ))
                }
            };
            let remapped = MethodHandle {
                handle_type: handle.handle_type,
                member: new_member,
            };
            let new_idx = if let Some(pos) = dest.method_handles.iter().position(|mh| *mh == remapped) {
                pos as u32
            } else {
                let idx = dest.method_handles.len() as u32;
                dest.method_handles.push(remapped);
                idx
            };
            mh_remap.insert(mh_idx, new_idx);
        }
    }

    // Transplant call_sites (dedup against existing entries in dest)
    let mut cs_remap: HashMap<u32, u32> = HashMap::new();
    for &cs_idx in &refs.call_sites {
        if let Some(cs) = source.call_sites.get(cs_idx as usize) {
            let bootstrap = mh_remap
                .get(&cs.bootstrap_method.0)
                .map(|&v| MethodHandleIdx(v))
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

            let new_idx = if let Some(pos) = dest.call_sites.iter().position(|existing| *existing == new_cs) {
                pos as u32
            } else {
                let idx = dest.call_sites.len() as u32;
                dest.call_sites.push(new_cs);
                idx
            };
            cs_remap.insert(cs_idx, new_idx);
        }
    }

    // Apply main remap to the class
    let remap = Remap {
        string: &string_remap,
        type_: &type_remap,
        proto: &proto_remap,
        field: &field_remap,
        method: &method_remap,
    };
    remap.remap_class(class);

    // Remap call_site/method_handle indices in instructions and encoded values
    if !cs_remap.is_empty() || !mh_remap.is_empty() {
        remap_exotic_refs(class, &cs_remap, &mh_remap);
    }

    Ok(())
}

/// Remap an encoded value using both the standard Remap and method_handle remap.
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

/// Remap call_site and method_handle indices within a class's instructions
/// and encoded values (which the standard Remap doesn't handle).
fn remap_exotic_refs(
    class: &mut ClassDef,
    cs_remap: &HashMap<u32, u32>,
    mh_remap: &HashMap<u32, u32>,
) {
    if let Some(data) = &mut class.class_data {
        for method in data
            .direct_methods
            .iter_mut()
            .chain(data.virtual_methods.iter_mut())
        {
            if let Some(code) = &mut method.code {
                for insn in &mut code.instructions {
                    match insn {
                        Instruction::InvokeCustom { call_site, .. }
                        | Instruction::InvokeCustomRange { call_site, .. } => {
                            if let Some(&new_cs) = cs_remap.get(&call_site.0) {
                                *call_site = CallSiteIdx(new_cs);
                            }
                        }
                        Instruction::ConstMethodHandle {
                            method_handle, ..
                        } => {
                            if let Some(&new_mh) = mh_remap.get(&method_handle.0) {
                                *method_handle = MethodHandleIdx(new_mh);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    for sv in &mut class.static_values {
        remap_mh_in_encoded_value(sv, mh_remap);
    }

    if let Some(ann) = &mut class.annotations {
        remap_mh_in_annotations_dir(ann, mh_remap);
    }
}

fn remap_mh_in_encoded_value(v: &mut EncodedValue, mh_remap: &HashMap<u32, u32>) {
    match v {
        EncodedValue::MethodHandle(idx) => {
            if let Some(&new_mh) = mh_remap.get(&idx.0) {
                *idx = MethodHandleIdx(new_mh);
            }
        }
        EncodedValue::Array(items) => {
            for item in items {
                remap_mh_in_encoded_value(item, mh_remap);
            }
        }
        EncodedValue::Annotation(ann) => {
            for elem in &mut ann.elements {
                remap_mh_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
        _ => {}
    }
}

fn remap_mh_in_annotations_dir(dir: &mut AnnotationsDirectory, mh_remap: &HashMap<u32, u32>) {
    for item in &mut dir.class_annotations {
        remap_mh_in_annotation_item(item, mh_remap);
    }
    for (_, items) in &mut dir.field_annotations {
        for item in items {
            remap_mh_in_annotation_item(item, mh_remap);
        }
    }
    for (_, items) in &mut dir.method_annotations {
        for item in items {
            remap_mh_in_annotation_item(item, mh_remap);
        }
    }
    for (_, param_items) in &mut dir.parameter_annotations {
        for items in param_items {
            for item in items {
                remap_mh_in_annotation_item(item, mh_remap);
            }
        }
    }
}

fn remap_mh_in_annotation_item(item: &mut AnnotationItem, mh_remap: &HashMap<u32, u32>) {
    for elem in &mut item.elements {
        remap_mh_in_encoded_value(&mut elem.value, mh_remap);
    }
}

// --- Remap builders (intern from source into dest) ---

fn build_string_remap(used: &HashSet<u32>, source: &DexFile, dest: &mut DexFile) -> Vec<u32> {
    let mut remap = vec![u32::MAX; source.strings.len()];
    for &si in used {
        let val = source.string(StringIdx(si));
        remap[si as usize] = dest.intern_string(val).0;
    }
    remap
}

fn build_type_remap(used: &HashSet<u32>, source: &DexFile, dest: &mut DexFile) -> Vec<u32> {
    let mut remap = vec![u32::MAX; source.types.len()];
    for &ti in used {
        let desc = source.type_descriptor(TypeIdx(ti));
        remap[ti as usize] = dest.intern_type(desc).0;
    }
    remap
}

fn build_proto_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.prototypes.len()];
    for &pi in used {
        let proto = &source.prototypes[pi as usize];
        let ret = source.type_descriptor(proto.return_type);
        let params: Vec<&str> = proto
            .parameters
            .iter()
            .map(|p| source.type_descriptor(*p))
            .collect();
        let desc = format!("({}){}", params.join(""), ret);
        remap[pi as usize] = dest.intern_proto(&desc)?.0 as u32;
    }
    Ok(remap)
}

fn build_method_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.methods.len()];
    for &mi in used {
        let m = &source.methods[mi as usize];
        let class_desc = source.type_descriptor(m.class);
        let name = source.string(m.name);
        let proto = &source.prototypes[m.proto.0 as usize];
        let ret = source.type_descriptor(proto.return_type);
        let params: Vec<&str> = proto
            .parameters
            .iter()
            .map(|p| source.type_descriptor(*p))
            .collect();
        let proto_desc = format!("({}){}", params.join(""), ret);
        remap[mi as usize] = dest.intern_method(class_desc, name, &proto_desc)?.0;
    }
    Ok(remap)
}

fn build_field_remap(
    used: &HashSet<u32>,
    source: &DexFile,
    dest: &mut DexFile,
) -> crate::error::Result<Vec<u32>> {
    let mut remap = vec![u32::MAX; source.fields.len()];
    for &fi in used {
        let f = &source.fields[fi as usize];
        let class_desc = source.type_descriptor(f.class);
        let name = source.string(f.name);
        let type_desc = source.type_descriptor(f.type_);
        remap[fi as usize] = dest.intern_field(class_desc, name, type_desc)?.0;
    }
    Ok(remap)
}

// --- Snapshot/rollback for overflow detection ---

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
        dex.build_lookups();
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

// --- Table compaction (removes unreferenced entries) ---

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
    let proto_remap_u32 = build_compact_remap(&refs.protos, dex.prototypes.len());
    let field_remap = build_compact_remap(&refs.fields, dex.fields.len());
    let method_remap = build_compact_remap(&refs.methods, dex.methods.len());
    let cs_remap = build_compact_remap(&refs.call_sites, dex.call_sites.len());
    let mh_remap = build_compact_remap(&refs.method_handles, dex.method_handles.len());

    let proto_remap: Vec<u16> = proto_remap_u32
        .iter()
        .map(|&v| if v == u32::MAX { u16::MAX } else { v as u16 })
        .collect();

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
        p.shorty = StringIdx(string_remap[p.shorty.0 as usize]);
        p.return_type = TypeIdx(type_remap[p.return_type.0 as usize]);
        for param in &mut p.parameters {
            param.0 = type_remap[param.0 as usize];
        }
    }
    for f in &mut dex.fields {
        f.class = TypeIdx(type_remap[f.class.0 as usize]);
        f.type_ = TypeIdx(type_remap[f.type_.0 as usize]);
        f.name = StringIdx(string_remap[f.name.0 as usize]);
    }
    for m in &mut dex.methods {
        m.class = TypeIdx(type_remap[m.class.0 as usize]);
        m.proto = ProtoIdx(proto_remap[m.proto.0 as usize]);
        m.name = StringIdx(string_remap[m.name.0 as usize]);
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

    dex.build_lookups();
}

fn remap_all_exotic_indices(
    dex: &mut DexFile,
    cs_remap: &[u32],
    mh_remap: &[u32],
) {
    for class in &mut dex.classes {
        if let Some(data) = &mut class.class_data {
            for method in data.direct_methods.iter_mut().chain(data.virtual_methods.iter_mut()) {
                if let Some(code) = &mut method.code {
                    for insn in &mut code.instructions {
                        match insn {
                            Instruction::InvokeCustom { call_site, .. }
                            | Instruction::InvokeCustomRange { call_site, .. } => {
                                if let Some(&new) = cs_remap.get(call_site.0 as usize) {
                                    if new != u32::MAX {
                                        call_site.0 = new;
                                    }
                                }
                            }
                            Instruction::ConstMethodHandle { method_handle, .. } => {
                                if let Some(&new) = mh_remap.get(method_handle.0 as usize) {
                                    if new != u32::MAX {
                                        method_handle.0 = new;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        for sv in &mut class.static_values {
            remap_mh_idx_in_encoded_value(sv, mh_remap);
        }
        if let Some(ann) = &mut class.annotations {
            remap_mh_idx_in_annotations_dir(ann, mh_remap);
        }
    }
}

fn remap_mh_idx_in_encoded_value(v: &mut EncodedValue, mh_remap: &[u32]) {
    match v {
        EncodedValue::MethodHandle(idx) => {
            if let Some(&new) = mh_remap.get(idx.0 as usize) {
                if new != u32::MAX {
                    idx.0 = new;
                }
            }
        }
        EncodedValue::Array(items) => {
            for item in items {
                remap_mh_idx_in_encoded_value(item, mh_remap);
            }
        }
        EncodedValue::Annotation(ann) => {
            for elem in &mut ann.elements {
                remap_mh_idx_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
        _ => {}
    }
}

fn remap_mh_idx_in_annotations_dir(dir: &mut AnnotationsDirectory, mh_remap: &[u32]) {
    for item in &mut dir.class_annotations {
        for elem in &mut item.elements {
            remap_mh_idx_in_encoded_value(&mut elem.value, mh_remap);
        }
    }
    for (_, items) in &mut dir.field_annotations {
        for item in items {
            for elem in &mut item.elements {
                remap_mh_idx_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
    }
    for (_, items) in &mut dir.method_annotations {
        for item in items {
            for elem in &mut item.elements {
                remap_mh_idx_in_encoded_value(&mut elem.value, mh_remap);
            }
        }
    }
    for (_, param_items) in &mut dir.parameter_annotations {
        for items in param_items {
            for item in items {
                for elem in &mut item.elements {
                    remap_mh_idx_in_encoded_value(&mut elem.value, mh_remap);
                }
            }
        }
    }
}

fn build_compact_remap(used: &HashSet<u32>, len: usize) -> Vec<u32> {
    let mut remap = vec![u32::MAX; len];
    let mut new_idx = 0u32;
    for i in 0..len as u32 {
        if used.contains(&i) {
            remap[i as usize] = new_idx;
            new_idx += 1;
        }
    }
    remap
}

fn filter_indexed<T: Clone>(items: &[T], used: &HashSet<u32>) -> Vec<T> {
    items
        .iter()
        .enumerate()
        .filter(|(i, _)| used.contains(&(*i as u32)))
        .map(|(_, item)| item.clone())
        .collect()
}
