// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use crate::file::DexFile;
use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::{ClassData, ClassDef};
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::instruction::Instruction;
use crate::types::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember};

pub(super) struct ReferencedIndices {
    pub(super) strings: HashSet<u32>,
    pub(super) types: HashSet<u32>,
    pub(super) protos: HashSet<u32>,
    pub(super) fields: HashSet<u32>,
    pub(super) methods: HashSet<u32>,
    pub(super) call_sites: HashSet<u32>,
    pub(super) method_handles: HashSet<u32>,
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

    pub(super) fn collect_from_dex(dex: &DexFile) -> Self {
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

    pub(super) fn collect_from_classes(classes: &[ClassDef]) -> Self {
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
            Instruction::ConstMethodHandle { method_handle, .. } => {
                self.method_handles.insert(method_handle.0);
            }
            _ => {}
        }
    }

    fn collect_debug(&mut self, debug: &DebugInfo) {
        for name in debug.parameter_names.iter().flatten() {
            self.strings.insert(name.0);
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
                DebugBytecode::SetFile { name: Some(name) } => {
                    self.strings.insert(name.0);
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

    fn collect_encoded_value(&mut self, value: &EncodedValue) {
        match value {
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

    fn collect_call_site(&mut self, call_site: &CallSiteItem) {
        self.strings.insert(call_site.method_name.0);
        self.protos.insert(call_site.method_type.0 as u32);
        self.method_handles.insert(call_site.bootstrap_method.0);
        for arg in &call_site.extra_arguments {
            self.collect_encoded_value(arg);
        }
    }

    fn collect_method_handle(&mut self, method_handle: &MethodHandle) {
        match &method_handle.member {
            MethodHandleMember::Field(idx) => {
                self.fields.insert(idx.0);
            }
            MethodHandleMember::Method(idx) => {
                self.methods.insert(idx.0);
            }
        }
    }

    pub(super) fn expand_transitive(&mut self, dex: &DexFile) {
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

        for method_index in self.methods.iter().copied().collect::<Vec<_>>() {
            let method = &dex.methods[method_index as usize];
            self.types.insert(method.class.0);
            self.strings.insert(method.name.0);
            self.protos.insert(method.proto.0 as u32);
        }

        for field_index in self.fields.iter().copied().collect::<Vec<_>>() {
            let field = &dex.fields[field_index as usize];
            self.types.insert(field.class.0);
            self.types.insert(field.type_.0);
            self.strings.insert(field.name.0);
        }

        for proto_index in self.protos.iter().copied().collect::<Vec<_>>() {
            let proto = &dex.prototypes[proto_index as usize];
            self.strings.insert(proto.shorty.0);
            self.types.insert(proto.return_type.0);
            for param in &proto.parameters {
                self.types.insert(param.0);
            }
        }

        for type_index in self.types.iter().copied().collect::<Vec<_>>() {
            self.strings.insert(dex.types[type_index as usize].0);
        }
    }
}
