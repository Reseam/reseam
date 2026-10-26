// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::RefSink;
use crate::error::Result;
use crate::file::DexFile;
use crate::instruction::Instruction;
use crate::model::Pool;
use crate::model::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::model::debug::{DebugBytecode, DebugInfo};
use crate::model::encoded_value::EncodedValue;
use crate::model::method_handle::{CallSiteItem, MethodHandleMember};

pub(crate) fn call_site(sink: &mut impl RefSink, call_site: &CallSiteItem) {
    sink.add(Pool::MethodHandle, call_site.bootstrap_method.0);
    sink.add(Pool::String, call_site.method_name.0);
    sink.add(Pool::Proto, call_site.method_type.0);
    for arg in &call_site.extra_arguments {
        encoded_value(sink, arg);
    }
}

/// The references of entry `idx` of `pool`.
pub(crate) fn id_entry(sink: &mut impl RefSink, dex: &DexFile, pool: Pool, idx: u32) -> Result<()> {
    let at = idx as usize;
    match pool {
        Pool::String => {}
        Pool::Type => sink.add(Pool::String, dex.types.get(at).0),
        Pool::Proto => {
            let proto = dex.prototypes.get(at);
            sink.add(Pool::String, proto.shorty.0);
            sink.add(Pool::Type, proto.return_type.0);
            for parameter in &proto.parameters {
                sink.add(Pool::Type, parameter.0);
            }
        }
        Pool::Field => {
            let field = dex.fields.get(at);
            sink.add(Pool::Type, field.class.0);
            sink.add(Pool::Type, field.type_.0);
            sink.add(Pool::String, field.name.0);
        }
        Pool::Method => {
            let method = dex.methods.get(at);
            sink.add(Pool::Type, method.class.0);
            sink.add(Pool::Proto, method.proto.0);
            sink.add(Pool::String, method.name.0);
        }
        Pool::CallSite => call_site(sink, &*dex.call_sites.get(at)?),
        Pool::MethodHandle => match dex.method_handles.get(at)?.member {
            MethodHandleMember::Field(field) => sink.add(Pool::Field, field.0),
            MethodHandleMember::Method(method) => sink.add(Pool::Method, method.0),
        },
    }
    Ok(())
}

pub(crate) fn instruction(sink: &mut impl RefSink, instruction: &Instruction) {
    for (pool, index) in instruction.indices() {
        sink.add(pool, index);
    }
}

pub(crate) fn debug_info(sink: &mut impl RefSink, debug: &DebugInfo) {
    for name in debug.parameter_names.iter().flatten() {
        sink.add(Pool::String, name.0);
    }
    for bytecode in &debug.bytecodes {
        let (name, type_) = match *bytecode {
            DebugBytecode::StartLocal { name, type_, .. } => (name, type_),
            DebugBytecode::StartLocalExtended {
                name,
                type_,
                signature,
                ..
            } => {
                if let Some(signature) = signature {
                    sink.add(Pool::String, signature.0);
                }
                (name, type_)
            }
            DebugBytecode::SetFile { name } => (name, None),
            _ => continue,
        };
        if let Some(string) = name {
            sink.add(Pool::String, string.0);
        }
        if let Some(type_) = type_ {
            sink.add(Pool::Type, type_.0);
        }
    }
}

pub(crate) fn annotations_dir(sink: &mut impl RefSink, dir: &AnnotationsDirectory) {
    for item in &dir.class {
        annotation_item(sink, item);
    }
    for (field, items) in &dir.fields {
        sink.add(Pool::Field, field.0);
        for item in items {
            annotation_item(sink, item);
        }
    }
    for (method, items) in &dir.methods {
        sink.add(Pool::Method, method.0);
        for item in items {
            annotation_item(sink, item);
        }
    }
    for (method, params) in &dir.parameters {
        sink.add(Pool::Method, method.0);
        params
            .iter()
            .flatten()
            .for_each(|item| annotation_item(sink, item));
    }
}

fn annotation_item(sink: &mut impl RefSink, item: &AnnotationItem) {
    sink.add(Pool::Type, item.type_.0);
    for element in &item.elements {
        sink.add(Pool::String, element.name.0);
        encoded_value(sink, &element.value);
    }
}

pub(crate) fn encoded_value(sink: &mut impl RefSink, value: &EncodedValue) {
    match value {
        EncodedValue::String(idx) => sink.add(Pool::String, idx.0),
        EncodedValue::Type(idx) => sink.add(Pool::Type, idx.0),
        EncodedValue::Field(idx) | EncodedValue::Enum(idx) => sink.add(Pool::Field, idx.0),
        EncodedValue::Method(idx) => sink.add(Pool::Method, idx.0),
        EncodedValue::MethodType(idx) => sink.add(Pool::Proto, idx.0),
        EncodedValue::MethodHandle(idx) => sink.add(Pool::MethodHandle, idx.0),
        EncodedValue::Array(items) => items.iter().for_each(|item| encoded_value(sink, item)),
        EncodedValue::Annotation(annotation) => {
            sink.add(Pool::Type, annotation.type_.0);
            for element in &annotation.elements {
                sink.add(Pool::String, element.name.0);
                encoded_value(sink, &element.value);
            }
        }
        _ => {}
    }
}
