// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::reseam_dex::{
    AccessFlags, DexFile, EncodedField, EncodedValue, Fingerprint, InstructionPattern,
};

use boltffi::export;

use crate::kotlin::types::{
    ClassInfo, EncodedVal, FieldInfo, FingerprintDef, FingerprintResult, MethodInfo,
};
use crate::kotlin::{with_ctx, with_handles};

#[export]
pub fn find_method(class_descriptor: String, method_name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let location = ctx.find_method(&class_descriptor, &method_name)?;
        Some(with_handles(|h| {
            h.alloc_method(
                location.dex_idx,
                location.class_idx,
                location.method_idx,
                location.is_virtual,
            )
        }))
    })
}

#[export]
pub fn find_method_by_name(name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let location = ctx.find_method_by_name(&name)?;
        Some(with_handles(|h| {
            h.alloc_method(
                location.dex_idx,
                location.class_idx,
                location.method_idx,
                location.is_virtual,
            )
        }))
    })
}

#[export]
pub fn find_methods_by_strings(strings: Vec<String>) -> Vec<u32> {
    let locations = with_ctx(|ctx| {
        let str_refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        ctx.find_methods_by_strings(&str_refs)
    });
    locations
        .into_iter()
        .map(|location| {
            with_handles(|h| {
                h.alloc_method(
                    location.dex_idx,
                    location.class_idx,
                    location.method_idx,
                    location.is_virtual,
                )
            })
        })
        .collect()
}

fn method_handles(locations: Vec<crate::context::MethodLocation>) -> Vec<u32> {
    with_handles(|h| {
        locations
            .into_iter()
            .map(|l| h.alloc_method(l.dex_idx, l.class_idx, l.method_idx, l.is_virtual))
            .collect()
    })
}

#[export]
pub fn find_methods_by_return_type(return_type: String) -> Vec<u32> {
    method_handles(with_ctx(|ctx| ctx.find_methods_by_return_type(&return_type)))
}

#[export]
pub fn find_methods_by_parameter_types(parameter_types: Vec<String>) -> Vec<u32> {
    method_handles(with_ctx(|ctx| {
        let types: Vec<&str> = parameter_types.iter().map(String::as_str).collect();
        ctx.find_methods_by_parameter_types(&types)
    }))
}

#[export]
pub fn find_methods_with_parameter(parameter_type: String) -> Vec<u32> {
    method_handles(with_ctx(|ctx| ctx.find_methods_with_parameter(&parameter_type)))
}

#[export]
pub fn find_methods_by_opcodes(pattern: Vec<i32>) -> Vec<u32> {
    let ip: Vec<InstructionPattern> = pattern
        .iter()
        .map(|o| {
            if *o < 0 {
                InstructionPattern::Any
            } else {
                InstructionPattern::OpcodeValue(*o as u16)
            }
        })
        .collect();
    let locations = with_ctx(|ctx| ctx.find_methods_with_opcodes(&ip));
    locations
        .into_iter()
        .map(|location| {
            with_handles(|h| {
                h.alloc_method(
                    location.dex_idx,
                    location.class_idx,
                    location.method_idx,
                    location.is_virtual,
                )
            })
        })
        .collect()
}

#[export]
pub fn find_method_by_fingerprint(fp: FingerprintDef) -> Option<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let fingerprint = with_ctx(|ctx| ctx.find_method_by_fingerprint(&dex_fp))?;
    let mh = with_handles(|h| {
        h.alloc_method(
            fingerprint.method.dex_idx,
            fingerprint.method.class_idx,
            fingerprint.method.method_idx,
            fingerprint.method.is_virtual,
        )
    });
    Some(FingerprintResult {
        method: mh,
        matched_count: fingerprint.matched_indices.len() as u32,
    })
}

#[export]
pub fn find_methods_by_fingerprint(fp: FingerprintDef) -> Vec<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let fingerprints = with_ctx(|ctx| ctx.find_methods_by_fingerprint(&dex_fp));
    fingerprints
        .into_iter()
        .map(|fingerprint| {
            let mh = with_handles(|h| {
                h.alloc_method(
                    fingerprint.method.dex_idx,
                    fingerprint.method.class_idx,
                    fingerprint.method.method_idx,
                    fingerprint.method.is_virtual,
                )
            });
            FingerprintResult {
                method: mh,
                matched_count: fingerprint.matched_indices.len() as u32,
            }
        })
        .collect()
}

#[export]
pub fn find_class(descriptor: String) -> Option<u32> {
    with_ctx(|ctx| {
        let location = ctx.find_class(&descriptor)?;
        Some(with_handles(|h| {
            h.alloc_class(location.dex_idx, location.class_idx)
        }))
    })
}

#[export]
pub fn get_all_classes() -> Vec<u32> {
    with_ctx(|ctx| {
        let mut handles = Vec::new();
        for dex_idx in 0..ctx.dex_count() {
            if let Some(dex) = ctx.dex_file(dex_idx) {
                for class_idx in 0..dex.classes.len() {
                    let h = with_handles(|h| h.alloc_class(dex_idx, class_idx));
                    handles.push(h);
                }
            }
        }
        handles
    })
}

#[export]
pub fn get_method_info(m: u32) -> Option<MethodInfo> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let method =
            ctx.read_method_summary(mh.dex_idx, mh.class_idx, mh.method_idx, mh.is_virtual)?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let class_type = dex.class_header(mh.class_idx).class_type;
        let method_id = dex.methods.try_get(method.method.0 as usize)?;
        Some(MethodInfo {
            class_descriptor: dex.type_descriptor(class_type).into_owned(),
            method_name: dex.string(method_id.name).into_owned(),
            proto: dex.proto_descriptor(&dex.prototypes.try_get(method_id.proto.0 as usize)?),
            access_flags: method.access_flags.bits(),
            dex_index: mh.dex_idx as u32,
            register_count: method.registers_size,
            ins_size: method.ins_size,
            outs_size: method.outs_size,
            instruction_count: method.instruction_count,
        })
    })
}

#[export]
pub fn get_class_info(c: u32) -> Option<ClassInfo> {
    with_ctx(|ctx| {
        let ch = with_handles(|h| h.get_class(c))?;
        let counts = ctx.read_class_counts(ch.dex_idx, ch.class_idx)?;
        let dex = ctx.dex_file(ch.dex_idx)?;
        let class = dex.class_header(ch.class_idx);
        let desc = dex.type_descriptor(class.class_type).to_string();
        let superclass = class.superclass.map(|s| dex.type_descriptor(s).to_string());
        let interfaces: Vec<String> = dex
            .classes
            .interfaces(ch.class_idx)
            .iter()
            .map(|i| dex.type_descriptor(*i).to_string())
            .collect();
        let source_file = class.source_file.map(|idx| dex.string(idx).to_string());
        Some(ClassInfo {
            descriptor: desc,
            access_flags: class.access_flags.bits(),
            superclass,
            interfaces,
            source_file,
            dex_index: ch.dex_idx as u32,
            direct_method_count: counts.direct_methods,
            virtual_method_count: counts.virtual_methods,
            static_field_count: counts.static_fields,
            instance_field_count: counts.instance_fields,
        })
    })
}

#[export]
pub fn class_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let counts = with_ctx(|ctx| ctx.read_class_counts(ch.dex_idx, ch.class_idx)).unwrap_or_default();
    let mut handles =
        Vec::with_capacity((counts.direct_methods + counts.virtual_methods) as usize);
    for i in 0..counts.direct_methods as usize {
        handles.push(with_handles(|h| {
            h.alloc_method(ch.dex_idx, ch.class_idx, i, false)
        }));
    }
    for i in 0..counts.virtual_methods as usize {
        handles.push(with_handles(|h| {
            h.alloc_method(ch.dex_idx, ch.class_idx, i, true)
        }));
    }
    handles
}

#[export]
pub fn class_direct_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let count = with_ctx(|ctx| ctx.read_class_counts(ch.dex_idx, ch.class_idx))
        .map(|counts| counts.direct_methods)
        .unwrap_or(0);
    (0..count as usize)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, false)))
        .collect()
}

#[export]
pub fn class_virtual_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let count = with_ctx(|ctx| ctx.read_class_counts(ch.dex_idx, ch.class_idx))
        .map(|counts| counts.virtual_methods)
        .unwrap_or(0);
    (0..count as usize)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, true)))
        .collect()
}

#[export]
pub fn class_fields(c: u32) -> Vec<FieldInfo> {
    class_fields_filtered(c, true, true)
}

#[export]
pub fn class_static_fields(c: u32) -> Vec<FieldInfo> {
    class_fields_filtered(c, true, false)
}

#[export]
pub fn class_instance_fields(c: u32) -> Vec<FieldInfo> {
    class_fields_filtered(c, false, true)
}

fn class_fields_filtered(c: u32, want_static: bool, want_instance: bool) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let Some(ch) = with_handles(|h| h.get_class(c)) else {
            return Vec::new();
        };
        let Some((dex, statics, instances)) = ctx.read_class_fields(ch.dex_idx, ch.class_idx) else {
            return Vec::new();
        };
        let Ok(static_values) = dex.class_static_values(ch.class_idx) else {
            return Vec::new();
        };
        let class_type = dex.class_header(ch.class_idx).class_type;
        collect_fields(dex, class_type, &static_values, &statics, &instances, want_static, want_instance)
    })
}

fn convert_fingerprint(fp: &FingerprintDef) -> Fingerprint {
    Fingerprint {
        name: fp.name.clone(),
        defining_class: fp.defining_class.clone(),
        access_flags: fp.access_flags.map(AccessFlags::from_bits_truncate),
        return_type: fp.return_type.clone(),
        parameters: fp.parameters.clone(),
        opcodes: fp.opcodes.as_ref().map(|ops| {
            ops.iter()
                .map(|o| {
                    if *o < 0 {
                        InstructionPattern::Any
                    } else {
                        InstructionPattern::OpcodeValue(*o as u16)
                    }
                })
                .collect()
        }),
        strings: fp.strings.clone(),
        literals: fp.literals.clone(),
    }
}

fn collect_fields(
    dex: &DexFile,
    class_type: reseam_apk::reseam_dex::TypeIdx,
    static_values: &[EncodedValue],
    static_fields: &[EncodedField],
    instance_fields: &[EncodedField],
    want_static: bool,
    want_instance: bool,
) -> Vec<FieldInfo> {
    let mut fields = Vec::new();
    if want_static {
        for (i, f) in static_fields.iter().enumerate() {
            let field_id = dex.field_id(f.field);
            let initial_value = static_values.get(i).and_then(|v| encoded_val_from_dex(v, dex));
            fields.push(FieldInfo {
                class_descriptor: dex.type_descriptor(class_type).to_string(),
                name: dex.string(field_id.name).to_string(),
                field_type: dex.type_descriptor(field_id.type_).to_string(),
                access_flags: f.access_flags.bits(),
                initial_value,
            });
        }
    }
    if want_instance {
        for f in instance_fields {
            let field_id = dex.field_id(f.field);
            fields.push(FieldInfo {
                class_descriptor: dex.type_descriptor(class_type).to_string(),
                name: dex.string(field_id.name).to_string(),
                field_type: dex.type_descriptor(field_id.type_).to_string(),
                access_flags: f.access_flags.bits(),
                initial_value: None,
            });
        }
    }
    fields
}

fn encoded_val_from_dex(val: &EncodedValue, dex: &DexFile) -> Option<EncodedVal> {
    Some(match val {
        EncodedValue::Null => EncodedVal::Null,
        EncodedValue::Boolean(b) => EncodedVal::BoolVal(*b),
        EncodedValue::Byte(v) => EncodedVal::ByteVal(*v),
        EncodedValue::Short(v) => EncodedVal::ShortVal(*v),
        EncodedValue::Char(v) => EncodedVal::CharVal(*v),
        EncodedValue::Int(v) => EncodedVal::IntVal(*v),
        EncodedValue::Long(v) => EncodedVal::LongVal(*v),
        EncodedValue::Float(v) => EncodedVal::FloatVal(*v),
        EncodedValue::Double(v) => EncodedVal::DoubleVal(*v),
        EncodedValue::String(idx) => EncodedVal::StringVal(dex.string(*idx).to_string()),
        EncodedValue::Type(idx) => EncodedVal::TypeVal(dex.type_descriptor(*idx).to_string()),
        _ => return None,
    })
}
