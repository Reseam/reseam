// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Links the extension classes a mutation refers to before it lands.

use super::handles::with_ctx;
use super::types::{FieldRef, Instruction, MethodRef};

/// The type descriptors making up a method prototype, return type last.
pub(super) fn proto_types(proto: &str) -> Vec<&str> {
    let mut types = Vec::new();
    let mut rest = proto.trim_start_matches('(');
    while !rest.is_empty() {
        if rest.starts_with(')') {
            rest = &rest[1..];
            continue;
        }
        let array = rest.len() - rest.trim_start_matches('[').len();
        let body = &rest[array..];
        let len = if body.starts_with('L') {
            body.find(';').map_or(body.len(), |end| end + 1)
        } else {
            1
        };
        types.push(&rest[..array + len]);
        rest = &rest[array + len..];
    }
    types
}

fn method_types(method: &MethodRef) -> impl Iterator<Item = &str> {
    std::iter::once(method.defining_class.as_str()).chain(proto_types(&method.proto))
}

fn field_types(field: &FieldRef) -> impl Iterator<Item = &str> {
    [field.defining_class.as_str(), field.field_type.as_str()].into_iter()
}

fn instruction_types(insn: &Instruction) -> Vec<&str> {
    match insn {
        Instruction::RegType(r) => vec![r.type_descriptor.as_str()],
        Instruction::RegField(r) => field_types(&r.field).collect(),
        Instruction::Invoke(r) => method_types(&r.method).collect(),
        Instruction::InvokeRange(r) => method_types(&r.method).collect(),
        Instruction::FilledArray(r) => vec![r.type_descriptor.as_str()],
        Instruction::FilledArrayRange(r) => vec![r.type_descriptor.as_str()],
        _ => Vec::new(),
    }
}

pub(super) fn link_instructions(insns: &[Instruction]) {
    with_ctx(|ctx| ctx.link_types(insns.iter().flat_map(instruction_types)));
}

pub(super) fn link_method(method: &MethodRef) {
    with_ctx(|ctx| ctx.link_types(method_types(method)));
}

pub(super) fn link_descriptors<'a>(descriptors: impl IntoIterator<Item = &'a str>) {
    with_ctx(|ctx| ctx.link_types(descriptors));
}

#[cfg(test)]
mod tests {
    use super::proto_types;

    #[test]
    fn splits_prototypes_into_descriptors() {
        assert_eq!(
            proto_types("(J[Ljava/lang/String;ZLjava/util/List;)[[I"),
            ["J", "[Ljava/lang/String;", "Z", "Ljava/util/List;", "[[I"]
        );
        assert_eq!(proto_types("()V"), ["V"]);
    }
}
