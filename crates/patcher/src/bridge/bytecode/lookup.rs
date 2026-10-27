// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::MethodKind;

use crate::bridge::run_state::checked;
use crate::error::{PatcherError, Result as PatcherResult};
use boltffi::export;
use reseam_dex::{DexFile, EncodedField, InstructionPattern, TypeIdx};

use crate::bridge::changes::changed;
use crate::bridge::dex::types::{ClassInfo, EncodedVal, FieldInfo, MethodInfo};
use crate::bridge::handles::{
    alloc_class, alloc_method, alloc_methods, class_location, method_location,
};
use crate::bridge::phase::with_ctx;
use crate::context::{ClassLocation, MethodLocation};

#[export]
pub fn app_entry_hook() -> Result<u32, String> {
    changed();
    #[expect(
        clippy::redundant_closure_for_method_calls,
        reason = "the closure accepts every context lifetime"
    )]
    crate::bridge::phase::try_with_ctx(|ctx| ctx.app_entry_hook())
        .flatten()
        .map(alloc_method)
        .map_err(|e| e.to_string())
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
                .type_descriptor(dex.classes().header(location.class_idx).class_type)
                .into_owned(),
            method_name: dex.strings().get(method_id.name).into_owned(),
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
        let class = dex.classes().header(location.class_idx);
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
            source_file: class
                .source_file
                .map(|idx| dex.strings().get(idx).into_owned()),
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
        let Some(name) = dex.strings().find(&name) else {
            return Vec::new();
        };
        let methods = |kind| -> Vec<reseam_dex::MethodIdx> {
            if let Some(class) = dex.classes().resident(location.class_idx) {
                class.class_data.as_ref().map_or_else(Vec::new, |data| {
                    data.methods(kind)
                        .iter()
                        .map(|method| method.method)
                        .collect()
                })
            } else {
                checked(dex.class_skeleton(location.class_idx)).map_or_else(Vec::new, |skeleton| {
                    skeleton
                        .methods(kind)
                        .iter()
                        .map(|method| method.method)
                        .collect()
                })
            }
        };
        let mut result = Vec::new();
        for kind in MethodKind::ALL {
            result.extend(
                methods(kind)
                    .into_iter()
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
        }
        result
    })
}

fn method_handles(c: u32, kind: MethodKind) -> Vec<u32> {
    let Some(class) = class_location(c) else {
        return Vec::new();
    };
    let count = with_ctx(|ctx| checked(ctx.read_class_counts(class)))
        .map_or(0, |counts| counts.methods(kind));
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
        let class_type = dex.classes().header(location.class_idx).class_type;
        fields
            .statics
            .iter()
            .map(|field| {
                let initial_value = static_values.get(&field.field).and_then(|v| {
                    checked(
                        crate::bridge::dex::convert::export_value(dex, Some(location.dex_idx), v)
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
        name: dex.strings().get(field_id.name).into_owned(),
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

#[export]
pub fn find_classes_extending(superclass: String) -> Vec<u32> {
    with_ctx(|ctx| checked(ctx.find_classes_extending(&superclass)))
        .into_iter()
        .map(alloc_class)
        .collect()
}

#[export]
pub fn find_classes_implementing(interface: String) -> Vec<u32> {
    with_ctx(|ctx| checked(ctx.find_classes_implementing(&interface)))
        .into_iter()
        .map(alloc_class)
        .collect()
}

#[export]
pub fn find_classes_with_instance_field(field_type: String) -> Vec<u32> {
    with_ctx(|ctx| checked(ctx.find_classes_with_instance_field(&field_type)))
        .into_iter()
        .map(alloc_class)
        .collect()
}
