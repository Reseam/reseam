// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::instruction::Instruction;
use crate::model::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::model::code::CodeItem;
use crate::model::debug::{DebugBytecode, DebugInfo};
use crate::model::encoded_value::EncodedValue;
use crate::model::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember};
use crate::model::{
    FieldIdx, MethodHandleIdx, MethodIdx, PerPool, Pool, ProtoIdx, StringIdx, TypeIdx,
};

/// Where each source pool index lands in the written pools; `u32::MAX` marks
/// an entry the output leaves out.
pub(crate) struct Remap(PerPool<Vec<u32>>);

impl Remap {
    pub(crate) fn new(tables: PerPool<Vec<u32>>) -> Self {
        Self(tables)
    }

    pub(crate) fn into_tables(self) -> PerPool<Vec<u32>> {
        self.0
    }

    /// The written index of a source index the output must hold.
    pub(crate) fn index(&self, pool: Pool, idx: u32) -> crate::Result<u32> {
        self.0[pool]
            .get(idx as usize)
            .copied()
            .filter(|&mapped| mapped != u32::MAX)
            .ok_or_else(|| {
                crate::error::invalid(
                    "pool remap",
                    format!("{pool:?} index {idx} is absent from the output pool"),
                )
            })
    }

    fn map(&self, pool: Pool, idx: u32) -> u32 {
        self.0[pool][idx as usize]
    }

    pub(crate) fn remap_string(&self, idx: StringIdx) -> StringIdx {
        StringIdx(self.map(Pool::String, idx.0))
    }

    pub(crate) fn remap_type(&self, idx: TypeIdx) -> TypeIdx {
        TypeIdx(self.map(Pool::Type, idx.0))
    }

    pub(crate) fn remap_proto(&self, idx: ProtoIdx) -> ProtoIdx {
        ProtoIdx(self.map(Pool::Proto, idx.0))
    }

    pub(crate) fn remap_field(&self, idx: FieldIdx) -> FieldIdx {
        FieldIdx(self.map(Pool::Field, idx.0))
    }

    pub(crate) fn remap_method(&self, idx: MethodIdx) -> MethodIdx {
        MethodIdx(self.map(Pool::Method, idx.0))
    }

    pub(crate) fn remap_method_handle_idx(&self, idx: MethodHandleIdx) -> MethodHandleIdx {
        MethodHandleIdx(self.map(Pool::MethodHandle, idx.0))
    }

    pub(crate) fn remap_opt_string(&self, idx: Option<StringIdx>) -> Option<StringIdx> {
        idx.map(|i| self.remap_string(i))
    }

    pub(crate) fn remap_opt_type(&self, idx: Option<TypeIdx>) -> Option<TypeIdx> {
        idx.map(|i| self.remap_type(i))
    }

    pub(crate) fn remap_code(&self, code: &mut CodeItem) -> crate::error::Result<()> {
        let mut validator = crate::references::ReferenceValidator::new(|pool, index| {
            self.index(pool, index).map(|_| ())
        });
        for instruction in &code.instructions {
            crate::references::instruction(&mut validator, instruction);
        }
        for handler in &code.catch_handlers {
            for catch in &handler.typed_catches {
                crate::references::RefSink::add(&mut validator, Pool::Type, catch.exception_type.0);
            }
        }
        if let Some(debug) = &code.debug_info {
            crate::references::debug_info(&mut validator, debug.read()?.as_ref());
        }
        validator.finish()?;

        for insn in &mut code.instructions {
            self.remap_instruction(insn);
        }
        for handler in &mut code.catch_handlers {
            for tc in &mut handler.typed_catches {
                tc.exception_type = self.remap_type(tc.exception_type);
            }
        }
        if let Some(ref mut debug) = code.debug_info {
            self.remap_debug(debug.resolve_mut()?)?;
        }
        Ok(())
    }

    pub(crate) fn remap_debug(&self, debug: &mut DebugInfo) -> crate::Result<()> {
        let mut validator = crate::references::ReferenceValidator::new(|pool, index| {
            self.index(pool, index).map(|_| ())
        });
        crate::references::debug_info(&mut validator, debug);
        validator.finish()?;
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
        Ok(())
    }

    pub(crate) fn remap_annotations_dir(
        &self,
        dir: &mut AnnotationsDirectory,
    ) -> crate::Result<()> {
        let mut validator = crate::references::ReferenceValidator::new(|pool, index| {
            self.index(pool, index).map(|_| ())
        });
        crate::references::annotations_dir(&mut validator, dir);
        validator.finish()?;
        for item in &mut dir.class {
            self.remap_annotation_item(item);
        }
        dir.class.sort_by_key(|item| item.type_.0);

        for (field_idx, items) in &mut dir.fields {
            *field_idx = self.remap_field(*field_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.fields.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, items) in &mut dir.methods {
            *method_idx = self.remap_method(*method_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.methods.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, param_items) in &mut dir.parameters {
            *method_idx = self.remap_method(*method_idx);
            for items in param_items.iter_mut() {
                for item in items.iter_mut() {
                    self.remap_annotation_item(item);
                }
                items.sort_by_key(|item| item.type_.0);
            }
        }
        dir.parameters.sort_by_key(|(idx, _)| idx.0);
        Ok(())
    }

    fn remap_annotation_item(&self, item: &mut AnnotationItem) {
        item.type_ = self.remap_type(item.type_);
        for elem in &mut item.elements {
            elem.name = self.remap_string(elem.name);
            self.map_value(&mut elem.value);
        }
        item.elements.sort_by_key(|e| e.name.0);
    }

    pub(crate) fn remap_encoded_value(&self, value: &mut EncodedValue) -> crate::Result<()> {
        let mut validator = crate::references::ReferenceValidator::new(|pool, index| {
            self.index(pool, index).map(|_| ())
        });
        crate::references::encoded_value(&mut validator, value);
        validator.finish()?;
        self.map_value(value);
        Ok(())
    }

    fn map_value(&self, v: &mut EncodedValue) {
        match v {
            EncodedValue::String(idx) => *idx = self.remap_string(*idx),
            EncodedValue::Type(idx) => *idx = self.remap_type(*idx),
            EncodedValue::Field(idx) | EncodedValue::Enum(idx) => *idx = self.remap_field(*idx),
            EncodedValue::Method(idx) => *idx = self.remap_method(*idx),
            EncodedValue::MethodType(idx) => *idx = self.remap_proto(*idx),
            EncodedValue::MethodHandle(idx) => *idx = self.remap_method_handle_idx(*idx),
            EncodedValue::Array(items) => {
                for item in items {
                    self.map_value(item);
                }
            }
            EncodedValue::Annotation(ann) => {
                ann.type_ = self.remap_type(ann.type_);
                for elem in &mut ann.elements {
                    elem.name = self.remap_string(elem.name);
                    self.map_value(&mut elem.value);
                }
                ann.elements.sort_by_key(|e| e.name.0);
            }
            _ => {}
        }
    }

    fn remap_instruction(&self, insn: &mut Instruction) {
        insn.map_indices(|pool, index| self.map(pool, index));
    }

    pub(crate) fn remap_call_site(&self, cs: &mut CallSiteItem) -> crate::Result<()> {
        let mut validator = crate::references::ReferenceValidator::new(|pool, index| {
            self.index(pool, index).map(|_| ())
        });
        crate::references::call_site(&mut validator, cs);
        validator.finish()?;
        cs.bootstrap_method = self.remap_method_handle_idx(cs.bootstrap_method);
        cs.method_name = self.remap_string(cs.method_name);
        cs.method_type = self.remap_proto(cs.method_type);
        for arg in &mut cs.extra_arguments {
            self.map_value(arg);
        }
        Ok(())
    }

    pub(crate) fn remap_method_handle(&self, mh: &mut MethodHandle) -> crate::Result<()> {
        match mh.member {
            MethodHandleMember::Field(index) => {
                self.index(Pool::Field, index.0)?;
            }
            MethodHandleMember::Method(index) => {
                self.index(Pool::Method, index.0)?;
            }
        }
        match &mut mh.member {
            MethodHandleMember::Field(idx) => *idx = self.remap_field(*idx),
            MethodHandleMember::Method(idx) => *idx = self.remap_method(*idx),
        }
        Ok(())
    }
}

pub(crate) fn fixup_code(code: &mut CodeItem) -> crate::error::Result<()> {
    let mut i = 0;
    while i < code.instructions.len() {
        if let Instruction::ConstString { dest, string } = &code.instructions[i]
            && string.0 > 0xFFFF
        {
            let promoted = Instruction::ConstStringJumbo {
                dest: *dest,
                string: *string,
            };
            code.replace_instructions(i, &[promoted])?;
        }
        i += 1;
    }
    Ok(())
}
