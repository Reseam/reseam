// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::{ClassData, ClassDef, EncodedMethod};
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::instruction::Instruction;
use crate::types::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember};
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

/// Owned index remap tables, retained after sorting so the writer can remap
/// classes it decodes lazily (never-materialized classes) at emit time.
pub(crate) struct RemapTables {
    pub string: Vec<u32>,
    pub type_: Vec<u32>,
    pub proto: Vec<u32>,
    pub field: Vec<u32>,
    pub method: Vec<u32>,
}

impl RemapTables {
    pub(crate) fn as_remap(&self) -> Remap<'_> {
        Remap {
            string: &self.string,
            type_: &self.type_,
            proto: &self.proto,
            field: &self.field,
            method: &self.method,
        }
    }
}

pub(crate) struct Remap<'a> {
    pub(crate) string: &'a [u32],
    pub(crate) type_: &'a [u32],
    pub(crate) proto: &'a [u32],
    pub(crate) field: &'a [u32],
    pub(crate) method: &'a [u32],
}

impl<'a> Remap<'a> {
    pub(crate) fn remap_string(&self, idx: StringIdx) -> StringIdx {
        StringIdx(self.string[idx.0 as usize])
    }

    pub(crate) fn remap_type(&self, idx: TypeIdx) -> TypeIdx {
        TypeIdx(self.type_[idx.0 as usize])
    }

    pub(crate) fn remap_proto(&self, idx: ProtoIdx) -> ProtoIdx {
        ProtoIdx(self.proto[idx.0 as usize] as u16)
    }

    pub(crate) fn remap_field(&self, idx: FieldIdx) -> FieldIdx {
        FieldIdx(self.field[idx.0 as usize])
    }

    pub(crate) fn remap_method(&self, idx: MethodIdx) -> MethodIdx {
        MethodIdx(self.method[idx.0 as usize])
    }

    pub(crate) fn remap_opt_string(&self, idx: Option<StringIdx>) -> Option<StringIdx> {
        idx.map(|i| self.remap_string(i))
    }

    pub(crate) fn remap_opt_type(&self, idx: Option<TypeIdx>) -> Option<TypeIdx> {
        idx.map(|i| self.remap_type(i))
    }

    pub(crate) fn remap_class(&self, class: &mut ClassDef) {
        class.class_type = self.remap_type(class.class_type);
        class.superclass = self.remap_opt_type(class.superclass);
        class.interfaces = class
            .interfaces
            .iter()
            .map(|t| self.remap_type(*t))
            .collect();
        class.source_file = self.remap_opt_string(class.source_file);

        if let Some(ref mut ann) = class.annotations {
            self.remap_annotations_dir(ann);
        }

        if let Some(ref mut data) = class.class_data {
            self.remap_class_data(data);
        }

        for sv in &mut class.static_values {
            self.remap_encoded_value(sv);
        }
    }

    pub(crate) fn remap_class_data(&self, data: &mut ClassData) {
        for f in &mut data.static_fields {
            f.field = self.remap_field(f.field);
        }
        for f in &mut data.instance_fields {
            f.field = self.remap_field(f.field);
        }
        // Re-sort fields by new index (delta encoding requires sorted order)
        data.static_fields.sort_by_key(|f| f.field.0);
        data.instance_fields.sort_by_key(|f| f.field.0);

        for m in &mut data.direct_methods {
            self.remap_encoded_method(m);
        }
        for m in &mut data.virtual_methods {
            self.remap_encoded_method(m);
        }
        // Re-sort methods by new index
        data.direct_methods.sort_by_key(|m| m.method.0);
        data.virtual_methods.sort_by_key(|m| m.method.0);
    }

    fn remap_encoded_method(&self, m: &mut EncodedMethod) {
        m.method = self.remap_method(m.method);
        if let Some(ref mut code) = m.code {
            self.remap_code(code);
        }
    }

    pub(crate) fn remap_code(&self, code: &mut CodeItem) {
        for insn in &mut code.instructions {
            self.remap_instruction(insn);
        }
        for handler in &mut code.catch_handlers {
            for tc in &mut handler.typed_catches {
                tc.exception_type = self.remap_type(tc.exception_type);
            }
        }
        if let Some(ref mut debug) = code.debug_info {
            self.remap_debug(debug);
        }
    }

    pub(crate) fn remap_debug(&self, debug: &mut DebugInfo) {
        for name in &mut debug.parameter_names {
            *name = self.remap_opt_string(*name);
        }
        for bc in &mut debug.bytecodes {
            match bc {
                DebugBytecode::StartLocal { name, type_, .. } => {
                    *name = self.remap_opt_string(*name);
                    *type_ = self.remap_opt_type(*type_);
                }
                DebugBytecode::StartLocalExtended {
                    name,
                    type_,
                    signature,
                    ..
                } => {
                    *name = self.remap_opt_string(*name);
                    *type_ = self.remap_opt_type(*type_);
                    *signature = self.remap_opt_string(*signature);
                }
                DebugBytecode::SetFile { name } => {
                    *name = self.remap_opt_string(*name);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn remap_annotations_dir(&self, dir: &mut AnnotationsDirectory) {
        for item in &mut dir.class_annotations {
            self.remap_annotation_item(item);
        }
        dir.class_annotations.sort_by_key(|item| item.type_.0);

        for (field_idx, items) in &mut dir.field_annotations {
            *field_idx = self.remap_field(*field_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.field_annotations.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, items) in &mut dir.method_annotations {
            *method_idx = self.remap_method(*method_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.method_annotations.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, param_items) in &mut dir.parameter_annotations {
            *method_idx = self.remap_method(*method_idx);
            for items in param_items.iter_mut() {
                for item in items.iter_mut() {
                    self.remap_annotation_item(item);
                }
                items.sort_by_key(|item| item.type_.0);
            }
        }
        dir.parameter_annotations.sort_by_key(|(idx, _)| idx.0);
    }

    fn remap_annotation_item(&self, item: &mut AnnotationItem) {
        item.type_ = self.remap_type(item.type_);
        for elem in &mut item.elements {
            elem.name = self.remap_string(elem.name);
            self.remap_encoded_value(&mut elem.value);
        }
        item.elements.sort_by_key(|e| e.name.0);
    }

    pub(crate) fn remap_encoded_value(&self, v: &mut EncodedValue) {
        match v {
            EncodedValue::String(idx) => *idx = self.remap_string(*idx),
            EncodedValue::Type(idx) => *idx = self.remap_type(*idx),
            EncodedValue::Field(idx) => *idx = self.remap_field(*idx),
            EncodedValue::Method(idx) => *idx = self.remap_method(*idx),
            EncodedValue::Enum(idx) => *idx = self.remap_field(*idx),
            EncodedValue::MethodType(idx) => *idx = self.remap_proto(*idx),
            EncodedValue::Array(items) => {
                for item in items {
                    self.remap_encoded_value(item);
                }
            }
            EncodedValue::Annotation(ann) => {
                ann.type_ = self.remap_type(ann.type_);
                for elem in &mut ann.elements {
                    elem.name = self.remap_string(elem.name);
                    self.remap_encoded_value(&mut elem.value);
                }
                ann.elements.sort_by_key(|e| e.name.0);
            }
            _ => {}
        }
    }

    fn remap_instruction(&self, insn: &mut crate::types::instruction::Instruction) {
        use crate::types::instruction::Instruction;
        match insn {
            Instruction::ConstString { string, .. }
            | Instruction::ConstStringJumbo { string, .. } => {
                *string = self.remap_string(*string);
            }
            Instruction::ConstClass { type_, .. }
            | Instruction::CheckCast { type_, .. }
            | Instruction::NewInstance { type_, .. } => {
                *type_ = self.remap_type(*type_);
            }
            Instruction::InstanceOf { type_, .. } | Instruction::NewArray { type_, .. } => {
                *type_ = self.remap_type(*type_);
            }
            Instruction::FilledNewArray { type_, .. }
            | Instruction::FilledNewArrayRange { type_, .. } => {
                *type_ = self.remap_type(*type_);
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
                *field = self.remap_field(*field);
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
                *method = self.remap_method(*method);
            }
            Instruction::InvokePolymorphic { method, proto, .. }
            | Instruction::InvokePolymorphicRange { method, proto, .. } => {
                *method = self.remap_method(*method);
                *proto = self.remap_proto(*proto);
            }
            Instruction::ConstMethodType { proto, .. } => {
                *proto = self.remap_proto(*proto);
            }
            // InvokeCustom/InvokeCustomRange reference call_site indices, not remapped
            // ConstMethodHandle references method_handle indices, not remapped
            _ => {}
        }
    }

    pub(crate) fn remap_call_site(&self, cs: &mut CallSiteItem) {
        cs.method_name = self.remap_string(cs.method_name);
        cs.method_type = self.remap_proto(cs.method_type);
        for arg in &mut cs.extra_arguments {
            self.remap_encoded_value(arg);
        }
    }

    pub(crate) fn remap_method_handle(&self, mh: &mut MethodHandle) {
        match &mut mh.member {
            MethodHandleMember::Field(idx) => *idx = self.remap_field(*idx),
            MethodHandleMember::Method(idx) => *idx = self.remap_method(*idx),
        }
    }
}

/// Promotes instructions whose remapped operand outgrew its encoded width
/// (currently `const-string` → `const-string/jumbo` when the string index
/// exceeds 16 bits). Shared by the resident-class fixup and the writer's lazy
/// class emitter so both paths widen identically.
pub(crate) fn fixup_code(code: &mut CodeItem) -> crate::error::Result<()> {
    let mut i = 0;
    while i < code.instructions.len() {
        if let Instruction::ConstString { dest, string } = &code.instructions[i] {
            if string.0 > 0xFFFF {
                let promoted = Instruction::ConstStringJumbo {
                    dest: *dest,
                    string: *string,
                };
                code.replace_instruction(i, promoted)?;
            }
        }
        i += 1;
    }
    Ok(())
}
