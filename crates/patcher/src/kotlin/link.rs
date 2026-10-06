// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::handles::with_ctx;
use super::types::{CallSiteRef, EncodedVal, FieldRef, HandleRef, Instruction, MethodRef};

pub(super) fn proto_types(proto: &str) -> Vec<&str> {
    let Some((parameters, returns)) =
        reseam_apk::reseam_dex::util::descriptor::parse_method_descriptor(proto)
    else {
        super::handles::record_failure(format!("invalid method descriptor: {proto}"));
        return Vec::new();
    };
    parameters
        .into_iter()
        .chain(std::iter::once(returns))
        .collect()
}

pub(super) fn method_types(method: &MethodRef) -> impl Iterator<Item = &str> {
    std::iter::once(method.defining_class.as_str()).chain(proto_types(&method.proto))
}

pub(super) fn field_types(field: &FieldRef) -> impl Iterator<Item = &str> {
    [field.defining_class.as_str(), field.field_type.as_str()].into_iter()
}

fn handle_types(handle: &HandleRef) -> Vec<&str> {
    match handle {
        HandleRef::Field(value) => field_types(&value.field).collect(),
        HandleRef::Method(value) => method_types(&value.method).collect(),
    }
}

pub(super) fn value_types(value: &EncodedVal) -> Vec<&str> {
    match value {
        EncodedVal::TypeVal(value) => vec![value.as_str()],
        EncodedVal::ProtoVal(value) => proto_types(value),
        EncodedVal::HandleVal(value) => handle_types(value),
        EncodedVal::FieldVal(value) | EncodedVal::EnumVal(value) => field_types(value).collect(),
        EncodedVal::MethodVal(value) => method_types(value).collect(),
        EncodedVal::ArrayVal(values) => values.iter().flat_map(value_types).collect(),
        EncodedVal::AnnotationVal(value) => std::iter::once(value.annotation_type.as_str())
            .chain(
                value
                    .elements
                    .iter()
                    .flat_map(|element| value_types(&element.value)),
            )
            .collect(),
        _ => Vec::new(),
    }
}

fn call_site_types(site: &CallSiteRef) -> Vec<&str> {
    handle_types(&site.bootstrap)
        .into_iter()
        .chain(proto_types(&site.proto))
        .chain(site.arguments.iter().flat_map(value_types))
        .collect()
}

fn instruction_types(insn: &Instruction) -> Vec<&str> {
    match insn {
        Instruction::RegType(r) => vec![r.type_descriptor.as_str()],
        Instruction::RegField(r) => field_types(&r.field).collect(),
        Instruction::Invoke(r) => method_types(&r.method).collect(),
        Instruction::InvokeRange(r) => method_types(&r.method).collect(),
        Instruction::Polymorphic(r) => method_types(&r.method)
            .chain(proto_types(&r.proto))
            .collect(),
        Instruction::PolymorphicRange(r) => method_types(&r.method)
            .chain(proto_types(&r.proto))
            .collect(),
        Instruction::Custom(r) => call_site_types(&r.call_site),
        Instruction::CustomRange(r) => call_site_types(&r.call_site),
        Instruction::RegHandle(r) => handle_types(&r.handle),
        Instruction::RegProto(r) => proto_types(&r.proto),
        Instruction::FilledArray(r) => vec![r.type_descriptor.as_str()],
        Instruction::FilledArrayRange(r) => vec![r.type_descriptor.as_str()],
        _ => Vec::new(),
    }
}

pub(super) fn link_instructions(insns: &[Instruction]) {
    link_descriptors(insns.iter().flat_map(instruction_types));
}

pub(super) fn link_method(method: &MethodRef) {
    link_descriptors(method_types(method));
}

pub(super) fn link_descriptors<'a>(descriptors: impl IntoIterator<Item = &'a str>) {
    with_ctx(|ctx| {
        let before = ctx.dex().len();
        ctx.link_types(descriptors);
        if ctx.dex().len() != before {
            super::handles::changed();
        }
    });
}
