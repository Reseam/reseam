// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Remap;
use crate::model::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::model::debug::{DebugBytecode, DebugInfo};
use crate::model::encoded_value::EncodedValue;
use crate::model::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember};
use crate::model::{Pool, StringIdx, TypeIdx};
use crate::references;

impl Remap {
    pub(crate) fn remap_debug(&self, debug: &mut DebugInfo) -> crate::Result<()> {
        self.check(|refs| {
            references::debug_info(refs, debug);
            Ok(())
        })?;
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
        self.check(|refs| {
            references::annotations_dir(refs, dir);
            Ok(())
        })?;
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
        self.check(|refs| {
            references::encoded_value(refs, value);
            Ok(())
        })?;
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

    pub(crate) fn remap_call_site(&self, cs: &mut CallSiteItem) -> crate::Result<()> {
        self.check(|refs| {
            references::call_site(refs, cs);
            Ok(())
        })?;
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

    fn remap_opt_string(&self, idx: Option<StringIdx>) -> Option<StringIdx> {
        idx.map(|i| self.remap_string(i))
    }

    fn remap_opt_type(&self, idx: Option<TypeIdx>) -> Option<TypeIdx> {
        idx.map(|i| self.remap_type(i))
    }
}
