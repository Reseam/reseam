// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use boltffi::export;
use reseam_apk::reseam_dex::{
    AccessFlags, DexFile, EncodedField, EncodedValue, Fingerprint, InstructionPattern, TypeIdx,
};

use crate::context::{ClassLocation, FingerprintLocation, MethodLocation};
use crate::kotlin::handles::{
    alloc_class, alloc_method, alloc_methods, class_location, method_location, with_ctx,
};
use crate::kotlin::types::{
    ClassInfo, EncodedVal, FieldInfo, FingerprintDef, FingerprintResult, MethodInfo,
};

#[export]
pub fn find_method(class_descriptor: String, method_name: String) -> Option<u32> {
    with_ctx(|ctx| ctx.find_method(&class_descriptor, &method_name)).map(alloc_method)
}

#[export]
pub fn find_method_by_name(name: String) -> Option<u32> {
    with_ctx(|ctx| ctx.find_method_by_name(&name)).map(alloc_method)
}

#[export]
pub fn find_methods_by_strings(strings: Vec<String>) -> Vec<u32> {
    let strings: Vec<&str> = strings.iter().map(String::as_str).collect();
    alloc_methods(with_ctx(|ctx| ctx.find_methods_by_strings(&strings)))
}

/// Methods whose prototype satisfies every given filter: exact return type,
/// exact parameter list, and a parameter of `parameter` type anywhere.
#[export]
pub fn find_methods_by_proto(
    return_type: Option<String>,
    parameter_types: Option<Vec<String>>,
    parameter: Option<String>,
) -> Vec<u32> {
    let parameter_types: Option<Vec<&str>> = parameter_types
        .as_ref()
        .map(|types| types.iter().map(String::as_str).collect());
    alloc_methods(with_ctx(|ctx| {
        ctx.find_methods_by_proto(
            return_type.as_deref(),
            parameter_types.as_deref(),
            parameter.as_deref(),
        )
    }))
}

/// Negative opcodes match any instruction.
#[export]
pub fn find_methods_by_opcodes(pattern: Vec<i32>) -> Vec<u32> {
    let pattern = opcode_patterns(&pattern);
    alloc_methods(with_ctx(|ctx| ctx.find_methods_with_opcodes(&pattern)))
}

#[export]
pub fn find_method_by_fingerprint(fp: FingerprintDef) -> Option<FingerprintResult> {
    let fingerprint = convert_fingerprint(&fp);
    with_ctx(|ctx| ctx.find_method_by_fingerprint(&fingerprint)).map(fingerprint_result)
}

#[export]
pub fn find_methods_by_fingerprint(fp: FingerprintDef) -> Vec<FingerprintResult> {
    let fingerprint = convert_fingerprint(&fp);
    with_ctx(|ctx| ctx.find_methods_by_fingerprint(&fingerprint))
        .into_iter()
        .map(fingerprint_result)
        .collect()
}

#[export]
pub fn find_class(descriptor: String) -> Option<u32> {
    with_ctx(|ctx| ctx.find_class(&descriptor)).map(alloc_class)
}

#[export]
pub fn get_all_classes() -> Vec<u32> {
    with_ctx(|ctx| {
        (0..ctx.dex().len())
            .flat_map(|dex_idx| {
                let classes = ctx.dex_file(dex_idx).map_or(0, |dex| dex.classes.len());
                (0..classes).map(move |class_idx| alloc_class(ClassLocation { dex_idx, class_idx }))
            })
            .collect()
    })
}

#[export]
pub fn get_method_info(m: u32) -> Option<MethodInfo> {
    let location = method_location(m)?;
    with_ctx(|ctx| {
        let summary = ctx.read_method_summary(location)?;
        let dex = ctx.dex_file(location.dex_idx)?;
        let method_id = dex.methods.try_get(summary.method.0 as usize)?;
        Some(MethodInfo {
            class_descriptor: dex
                .type_descriptor(dex.class_header(location.class_idx).class_type)
                .into_owned(),
            method_name: dex.string(method_id.name).into_owned(),
            proto: dex.proto_descriptor(&dex.prototypes.try_get(method_id.proto.0 as usize)?),
            access_flags: summary.access_flags.bits(),
            dex_index: location.dex_idx as u32,
            register_count: summary.registers_size,
            ins_size: summary.ins_size,
            outs_size: summary.outs_size,
            instruction_count: summary.instruction_count,
        })
    })
}

#[export]
pub fn get_class_info(c: u32) -> Option<ClassInfo> {
    let location = class_location(c)?;
    with_ctx(|ctx| {
        let counts = ctx.read_class_counts(location)?;
        let dex = ctx.dex_file(location.dex_idx)?;
        let class = dex.class_header(location.class_idx);
        Some(ClassInfo {
            descriptor: dex.type_descriptor(class.class_type).into_owned(),
            access_flags: class.access_flags.bits(),
            superclass: class
                .superclass
                .map(|s| dex.type_descriptor(s).into_owned()),
            interfaces: dex
                .classes
                .interfaces(location.class_idx)
                .iter()
                .map(|i| dex.type_descriptor(*i).into_owned())
                .collect(),
            source_file: class.source_file.map(|idx| dex.string(idx).into_owned()),
            dex_index: location.dex_idx as u32,
            direct_method_count: counts.direct_methods,
            virtual_method_count: counts.virtual_methods,
            static_field_count: counts.static_fields,
            instance_field_count: counts.instance_fields,
        })
    })
}

#[export]
pub fn class_direct_methods(c: u32) -> Vec<u32> {
    method_handles(c, false)
}

#[export]
pub fn class_virtual_methods(c: u32) -> Vec<u32> {
    method_handles(c, true)
}

fn method_handles(c: u32, is_virtual: bool) -> Vec<u32> {
    let Some(class) = class_location(c) else {
        return Vec::new();
    };
    let count = with_ctx(|ctx| ctx.read_class_counts(class)).map_or(0, |counts| {
        if is_virtual {
            counts.virtual_methods
        } else {
            counts.direct_methods
        }
    });
    alloc_methods((0..count as usize).map(|method_idx| MethodLocation {
        dex_idx: class.dex_idx,
        class_idx: class.class_idx,
        method_idx,
        is_virtual,
    }))
}

/// Static fields first, then instance fields.
#[export]
pub fn class_fields(c: u32) -> Vec<FieldInfo> {
    let Some(location) = class_location(c) else {
        return Vec::new();
    };
    with_ctx(|ctx| {
        let Some((dex, statics, instances)) = ctx.read_class_fields(location) else {
            return Vec::new();
        };
        let Ok(static_values) = dex.class_static_values(location.class_idx) else {
            return Vec::new();
        };
        let class_type = dex.class_header(location.class_idx).class_type;
        statics
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let initial_value = static_values.get(i).and_then(|v| encoded_val(v, dex));
                field_info(dex, class_type, field, initial_value)
            })
            .chain(
                instances
                    .iter()
                    .map(|field| field_info(dex, class_type, field, None)),
            )
            .collect()
    })
}

fn field_info(
    dex: &DexFile,
    class_type: TypeIdx,
    field: &EncodedField,
    initial_value: Option<EncodedVal>,
) -> FieldInfo {
    let field_id = dex.field_id(field.field);
    FieldInfo {
        class_descriptor: dex.type_descriptor(class_type).into_owned(),
        name: dex.string(field_id.name).into_owned(),
        field_type: dex.type_descriptor(field_id.type_).into_owned(),
        access_flags: field.access_flags.bits(),
        initial_value,
    }
}

pub(super) fn opcode_patterns(opcodes: &[i32]) -> Vec<InstructionPattern> {
    opcodes
        .iter()
        .map(|&op| {
            u16::try_from(op).map_or(InstructionPattern::Any, InstructionPattern::OpcodeValue)
        })
        .collect()
}

fn convert_fingerprint(fp: &FingerprintDef) -> Fingerprint {
    Fingerprint {
        name: fp.name.clone(),
        defining_class: fp.defining_class.clone(),
        access_flags: fp.access_flags.map(AccessFlags::from_bits_truncate),
        return_type: fp.return_type.clone(),
        parameters: fp.parameters.clone(),
        opcodes: fp.opcodes.as_deref().map(opcode_patterns),
        strings: fp.strings.clone(),
        literals: fp.literals.clone(),
    }
}

fn fingerprint_result(hit: FingerprintLocation) -> FingerprintResult {
    FingerprintResult {
        method: alloc_method(hit.method),
        matched_count: hit.matched_indices.len() as u32,
    }
}

fn encoded_val(value: &EncodedValue, dex: &DexFile) -> Option<EncodedVal> {
    Some(match value {
        EncodedValue::Null => EncodedVal::Null,
        EncodedValue::Boolean(b) => EncodedVal::BoolVal(*b),
        EncodedValue::Byte(v) => EncodedVal::ByteVal(*v),
        EncodedValue::Short(v) => EncodedVal::ShortVal(*v),
        EncodedValue::Char(v) => EncodedVal::CharVal(*v),
        EncodedValue::Int(v) => EncodedVal::IntVal(*v),
        EncodedValue::Long(v) => EncodedVal::LongVal(*v),
        EncodedValue::Float(v) => EncodedVal::FloatVal(*v),
        EncodedValue::Double(v) => EncodedVal::DoubleVal(*v),
        EncodedValue::String(idx) => EncodedVal::StringVal(dex.string(*idx).into_owned()),
        EncodedValue::Type(idx) => EncodedVal::TypeVal(dex.type_descriptor(*idx).into_owned()),
        _ => return None,
    })
}
