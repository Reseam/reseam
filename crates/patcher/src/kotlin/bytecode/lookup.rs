// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::reseam_dex::MethodKind;

use crate::error::{PatcherError, Result as PatcherResult};
use crate::kotlin::handles::checked;
use boltffi::export;
use reseam_apk::reseam_dex::{
    AccessFlags, DexFile, EncodedField, Fingerprint, InstructionPattern, TypeIdx,
};

use crate::context::{ClassLocation, FingerprintLocation, MethodLocation};
use crate::kotlin::handles::{
    alloc_class, alloc_method, alloc_methods, changed, class_location, method_location, with_ctx,
};
use crate::kotlin::types::{
    ClassInfo, EncodedVal, FieldInfo, FingerprintDef, FingerprintResult, MethodInfo,
};

#[export]
pub fn find_method(class_descriptor: String, method_name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let before = ctx.dex().len();
        ctx.find_or_link_class(&class_descriptor)?;
        if ctx.dex().len() != before {
            changed();
        }
        checked(ctx.find_method(&class_descriptor, &method_name))
    })
    .map(alloc_method)
}

#[export]
pub fn app_entry_hook() -> Result<u32, String> {
    changed();
    #[expect(
        clippy::redundant_closure_for_method_calls,
        reason = "the closure accepts every context lifetime"
    )]
    crate::kotlin::handles::try_with_ctx(|ctx| ctx.app_entry_hook())
        .flatten()
        .map(alloc_method)
        .map_err(|e| e.to_string())
}

#[export]
pub fn find_method_by_name(name: String) -> Option<u32> {
    with_ctx(|ctx| checked(ctx.find_method_by_name(&name))).map(alloc_method)
}

#[export]
pub fn find_methods_by_name(name: String) -> Vec<u32> {
    alloc_methods(with_ctx(|ctx| checked(ctx.find_methods_by_name(&name))))
}

#[export]
pub fn find_methods_by_strings(strings: Vec<String>) -> Vec<u32> {
    let strings: Vec<&str> = strings.iter().map(String::as_str).collect();
    alloc_methods(with_ctx(|ctx| {
        checked(ctx.find_methods_by_strings(&strings))
    }))
}

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
        checked(ctx.find_methods_by_proto(
            return_type.as_deref(),
            parameter_types.as_deref(),
            parameter.as_deref(),
        ))
    }))
}

#[export]
pub fn find_methods_by_opcodes(pattern: Vec<i32>) -> Vec<u32> {
    alloc_methods(with_ctx(|ctx| {
        checked(
            opcode_patterns(&pattern).and_then(|pattern| ctx.find_methods_with_opcodes(&pattern)),
        )
    }))
}

#[export]
pub fn find_method_by_fingerprint(fp: FingerprintDef) -> Option<FingerprintResult> {
    with_ctx(|ctx| {
        checked(
            convert_fingerprint(&fp)
                .and_then(|fingerprint| ctx.find_method_by_fingerprint(&fingerprint)),
        )
    })
    .map(fingerprint_result)
}

#[export]
pub fn find_methods_by_fingerprint(fp: FingerprintDef) -> Vec<FingerprintResult> {
    with_ctx(|ctx| {
        checked(
            convert_fingerprint(&fp)
                .and_then(|fingerprint| ctx.find_methods_by_fingerprint(&fingerprint)),
        )
    })
    .into_iter()
    .map(fingerprint_result)
    .collect()
}

#[export]
pub fn find_class(descriptor: String) -> Option<u32> {
    with_ctx(|ctx| {
        let before = ctx.dex().len();
        let class = ctx.find_or_link_class(&descriptor);
        if ctx.dex().len() != before {
            changed();
        }
        class
    })
    .map(alloc_class)
}

#[export]
pub fn get_all_classes() -> Vec<u32> {
    with_ctx(|ctx| {
        (0..ctx.dex().len())
            .flat_map(|dex_idx| {
                let classes = ctx.dex_file(dex_idx).map_or(0, |dex| dex.classes().len());
                (0..classes).map(move |class_idx| alloc_class(ClassLocation { dex_idx, class_idx }))
            })
            .collect()
    })
}

#[export]
pub fn get_method_info(m: u32) -> Option<MethodInfo> {
    let location = method_location(m)?;
    with_ctx(|ctx| {
        let summary = checked(ctx.read_method_summary(location))?;
        let dex = ctx.dex_file(location.dex_idx)?;
        let method_id = dex.methods().try_get(summary.method.0 as usize)?;
        Some(MethodInfo {
            class_descriptor: dex
                .type_descriptor(dex.class_header(location.class_idx).class_type)
                .into_owned(),
            method_name: dex.string(method_id.name).into_owned(),
            proto: dex.proto_descriptor(&dex.prototypes().try_get(method_id.proto.0 as usize)?),
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
        let counts = checked(ctx.read_class_counts(location))?;
        let dex = ctx.dex_file(location.dex_idx)?;
        let class = dex.class_header(location.class_idx);
        Some(ClassInfo {
            descriptor: dex.type_descriptor(class.class_type).into_owned(),
            access_flags: class.access_flags.bits(),
            superclass: class
                .superclass
                .map(|s| dex.type_descriptor(s).into_owned()),
            interfaces: dex
                .classes()
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
pub fn get_method_infos(methods: Vec<u32>) -> Vec<MethodInfo> {
    methods.into_iter().filter_map(get_method_info).collect()
}

#[export]
pub fn get_class_infos(classes: Vec<u32>) -> Vec<ClassInfo> {
    classes.into_iter().filter_map(get_class_info).collect()
}

#[export]
pub fn class_direct_methods(c: u32) -> Vec<u32> {
    method_handles(c, MethodKind::Direct)
}

#[export]
pub fn class_virtual_methods(c: u32) -> Vec<u32> {
    method_handles(c, MethodKind::Virtual)
}

#[export]
pub fn class_methods_by_name(c: u32, name: String) -> Vec<u32> {
    let Some(location) = class_location(c) else {
        return Vec::new();
    };
    with_ctx(|ctx| {
        let Some(dex) = ctx.dex_file(location.dex_idx) else {
            return Vec::new();
        };
        let Some(name) = dex.find_string_idx(&name) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut collect =
            |kind, methods: &mut dyn Iterator<Item = reseam_apk::reseam_dex::MethodIdx>| {
                result.extend(
                    methods
                        .enumerate()
                        .filter(|(_, method)| dex.method_id(*method).name == name)
                        .map(|(method_idx, _)| {
                            alloc_method(MethodLocation {
                                dex_idx: location.dex_idx,
                                class_idx: location.class_idx,
                                method_idx,
                                kind,
                            })
                        }),
                );
            };
        if let Some(class) = dex.resident_class(location.class_idx) {
            if let Some(data) = class.class_data.as_ref() {
                collect(
                    MethodKind::Direct,
                    &mut data.direct_methods.iter().map(|method| method.method),
                );
                collect(
                    MethodKind::Virtual,
                    &mut data.virtual_methods.iter().map(|method| method.method),
                );
            }
        } else if let Some(skeleton) = checked(dex.class_skeleton(location.class_idx)) {
            collect(
                MethodKind::Direct,
                &mut skeleton.direct_methods.iter().map(|method| method.method),
            );
            collect(
                MethodKind::Virtual,
                &mut skeleton.virtual_methods.iter().map(|method| method.method),
            );
        }
        result
    })
}

fn method_handles(c: u32, kind: MethodKind) -> Vec<u32> {
    let Some(class) = class_location(c) else {
        return Vec::new();
    };
    let count = with_ctx(|ctx| checked(ctx.read_class_counts(class))).map_or(0, |counts| {
        if kind == MethodKind::Virtual {
            counts.virtual_methods
        } else {
            counts.direct_methods
        }
    });
    alloc_methods((0..count as usize).map(|method_idx| MethodLocation {
        dex_idx: class.dex_idx,
        class_idx: class.class_idx,
        method_idx,
        kind,
    }))
}

#[export]
pub fn class_fields(c: u32) -> Vec<FieldInfo> {
    let Some(location) = class_location(c) else {
        return Vec::new();
    };
    with_ctx(|ctx| {
        let Some((dex, fields)) = checked(ctx.read_class_fields(location)) else {
            return Vec::new();
        };
        let Some(static_values) = checked(dex.class_static_values(location.class_idx).map(Some))
        else {
            return Vec::new();
        };
        let class_type = dex.class_header(location.class_idx).class_type;
        fields
            .statics
            .iter()
            .map(|field| {
                let initial_value = static_values.get(&field.field).and_then(|v| {
                    checked(
                        crate::kotlin::convert::export_value(dex, Some(location.dex_idx), v)
                            .map(Some),
                    )
                });
                field_info(dex, class_type, field, initial_value)
            })
            .chain(
                fields
                    .instances
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

pub(super) fn query_opcode(opcode: i32) -> PatcherResult<Option<u16>> {
    if opcode < 0 {
        return Ok(None);
    }
    u16::try_from(opcode)
        .map(Some)
        .map_err(|_| PatcherError::Bridge(format!("opcode filter {opcode} exceeds 16 bits")))
}

pub(super) fn opcode_patterns(opcodes: &[i32]) -> PatcherResult<Vec<InstructionPattern>> {
    opcodes
        .iter()
        .map(|&op| {
            query_opcode(op).map(|opcode| {
                opcode.map_or(InstructionPattern::Any, InstructionPattern::OpcodeValue)
            })
        })
        .collect()
}

fn convert_fingerprint(fp: &FingerprintDef) -> PatcherResult<Fingerprint> {
    Ok(Fingerprint {
        name: fp.name.clone(),
        defining_class: fp.defining_class.clone(),
        access_flags: fp.access_flags.map(AccessFlags::from_bits_retain),
        return_type: fp
            .return_type
            .clone()
            .map(reseam_apk::reseam_dex::TypePattern::Prefix),
        parameters: fp.parameters.as_ref().map(|parameters| {
            parameters
                .iter()
                .map(|parameter| match parameter.as_str() {
                    "L" => reseam_apk::reseam_dex::TypePattern::Object,
                    "[" => reseam_apk::reseam_dex::TypePattern::Array,
                    _ => reseam_apk::reseam_dex::TypePattern::Exact(parameter.clone()),
                })
                .collect()
        }),
        opcodes: fp.opcodes.as_deref().map(opcode_patterns).transpose()?,
        strings: fp.strings.clone(),
        literals: fp.literals.clone(),
    })
}

fn fingerprint_result(hit: FingerprintLocation) -> FingerprintResult {
    FingerprintResult {
        method: alloc_method(hit.method),
        matched_count: hit.matched_indices.len() as u32,
    }
}

#[export]
pub fn find_classes_with_instance_field(field_type: String) -> Vec<u32> {
    with_ctx(|ctx| checked(ctx.find_classes_with_instance_field(&field_type)))
        .into_iter()
        .map(alloc_class)
        .collect()
}
