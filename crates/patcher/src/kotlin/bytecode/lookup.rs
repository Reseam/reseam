use stitch_apk::stitch_dex::{AccessFlags, DexFile, EncodedValue, Fingerprint, InstructionPattern};

use boltffi::export;

use crate::kotlin::types::{
    ClassInfo, EncodedVal, FieldInfo, FingerprintDef, FingerprintResult, MethodInfo,
};
use crate::kotlin::{
    find_method_location, get_method_ref, method_match_location, with_ctx, with_handles,
};

#[export]
pub fn find_method(class_descriptor: String, method_name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let result = ctx.find_method(&class_descriptor, &method_name);
        match result {
            Some((dex_idx, method)) => {
                let (ci, mi, iv) = find_method_location(ctx, dex_idx, method)?;
                Some(with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv)))
            }
            None => None,
        }
    })
}

#[export]
pub fn find_method_by_name(name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let result = ctx.find_method_by_name(&name);
        match result {
            Some((dex_idx, mm)) => {
                let (ci, mi, iv) = method_match_location(ctx, dex_idx, &mm)?;
                Some(with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv)))
            }
            None => None,
        }
    })
}

#[export]
pub fn find_methods_by_strings(strings: Vec<String>) -> Vec<u32> {
    let locations: Vec<(usize, usize, usize, bool)> = with_ctx(|ctx| {
        let str_refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        let matches = ctx.find_methods_by_strings(&str_refs);
        matches
            .iter()
            .filter_map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm)?;
                Some((*dex_idx, ci, mi, iv))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv)| with_handles(|h| h.alloc_method(di, ci, mi, iv)))
        .collect()
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
    let locations: Vec<(usize, usize, usize, bool)> = with_ctx(|ctx| {
        let matches = ctx.find_methods_with_opcodes(&ip);
        matches
            .iter()
            .filter_map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm)?;
                Some((*dex_idx, ci, mi, iv))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv)| with_handles(|h| h.alloc_method(di, ci, mi, iv)))
        .collect()
}

#[export]
pub fn find_method_by_fingerprint(fp: FingerprintDef) -> Option<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let (dex_idx, ci, mi, iv, matched_count) = with_ctx(|ctx| {
        let (dex_idx, fm) = ctx.find_method_by_fingerprint(&dex_fp)?;
        let (ci, mi, iv) = find_method_location(ctx, dex_idx, fm.method)?;
        Some((dex_idx, ci, mi, iv, fm.matched_indices.len() as u32))
    })?;
    let mh = with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv));
    Some(FingerprintResult {
        method: mh,
        matched_count,
    })
}

#[export]
pub fn find_methods_by_fingerprint(fp: FingerprintDef) -> Vec<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let locs: Vec<(usize, usize, usize, bool, u32)> = with_ctx(|ctx| {
        let matches = ctx.find_methods_by_fingerprint(&dex_fp);
        matches
            .iter()
            .filter_map(|(dex_idx, fm)| {
                let (ci, mi, iv) = find_method_location(ctx, *dex_idx, fm.method)?;
                Some((*dex_idx, ci, mi, iv, fm.matched_indices.len() as u32))
            })
            .collect()
    });
    locs.into_iter()
        .map(|(di, ci, mi, iv, count)| {
            let mh = with_handles(|h| h.alloc_method(di, ci, mi, iv));
            FingerprintResult {
                method: mh,
                matched_count: count,
            }
        })
        .collect()
}

#[export]
pub fn find_class(descriptor: String) -> Option<u32> {
    with_ctx(|ctx| match ctx.find_class(&descriptor) {
        Some((dex_idx, class)) => {
            let dex = ctx.dex_file(dex_idx)?;
            let class_idx = dex.classes.iter().position(|c| std::ptr::eq(c, class))?;
            Some(with_handles(|h| h.alloc_class(dex_idx, class_idx)))
        }
        None => None,
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
        let dex = ctx.dex_file(mh.dex_idx)?;
        let class = dex.classes.get(mh.class_idx)?;
        let method = get_method_ref(dex, mh)?;
        let method_id = dex.methods.get(method.method.0 as usize)?;
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let name = dex.string(method_id.name).to_string();
        let proto_def = dex.prototypes.get(method_id.proto.0 as usize)?;
        let ret = dex.type_descriptor(proto_def.return_type);
        let params: Vec<&str> = proto_def
            .parameters
            .iter()
            .map(|p| dex.type_descriptor(*p))
            .collect();
        let proto = format!("({}){}", params.join(""), ret);
        let (reg_count, ins, outs, insn_count) = match &method.code {
            Some(c) => (
                c.registers_size,
                c.ins_size,
                c.outs_size,
                c.instructions.len() as u32,
            ),
            None => (0, 0, 0, 0),
        };
        Some(MethodInfo {
            class_descriptor: class_desc,
            method_name: name,
            proto,
            access_flags: method.access_flags.bits(),
            dex_index: mh.dex_idx as u32,
            register_count: reg_count,
            ins_size: ins,
            outs_size: outs,
            instruction_count: insn_count,
        })
    })
}

#[export]
pub fn get_class_info(c: u32) -> Option<ClassInfo> {
    with_ctx(|ctx| {
        let ch = with_handles(|h| h.get_class(c))?;
        let dex = ctx.dex_file(ch.dex_idx)?;
        let class = dex.classes.get(ch.class_idx)?;
        let desc = dex.type_descriptor(class.class_type).to_string();
        let superclass = class.superclass.map(|s| dex.type_descriptor(s).to_string());
        let interfaces: Vec<String> = class
            .interfaces
            .iter()
            .map(|i| dex.type_descriptor(*i).to_string())
            .collect();
        let source_file = class.source_file.map(|idx| dex.string(idx).to_string());
        let (dm, vm, sf, inf) = match &class.class_data {
            Some(d) => (
                d.direct_methods.len() as u32,
                d.virtual_methods.len() as u32,
                d.static_fields.len() as u32,
                d.instance_fields.len() as u32,
            ),
            None => (0, 0, 0, 0),
        };
        Some(ClassInfo {
            descriptor: desc,
            access_flags: class.access_flags.bits(),
            superclass,
            interfaces,
            source_file,
            dex_index: ch.dex_idx as u32,
            direct_method_count: dm,
            virtual_method_count: vm,
            static_field_count: sf,
            instance_field_count: inf,
        })
    })
}

#[export]
pub fn class_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let (dm_count, vm_count) = with_ctx(|ctx| {
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return (0, 0),
        };
        match dex
            .classes
            .get(ch.class_idx)
            .and_then(|c| c.class_data.as_ref())
        {
            Some(d) => (d.direct_methods.len(), d.virtual_methods.len()),
            None => (0, 0),
        }
    });
    let mut handles = Vec::with_capacity(dm_count + vm_count);
    for i in 0..dm_count {
        handles.push(with_handles(|h| {
            h.alloc_method(ch.dex_idx, ch.class_idx, i, false)
        }));
    }
    for i in 0..vm_count {
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
    let dm_count = with_ctx(|ctx| {
        ctx.dex_file(ch.dex_idx)
            .and_then(|dex| dex.classes.get(ch.class_idx))
            .and_then(|c| c.class_data.as_ref())
            .map(|d| d.direct_methods.len())
            .unwrap_or(0)
    });
    (0..dm_count)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, false)))
        .collect()
}

#[export]
pub fn class_virtual_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let vm_count = with_ctx(|ctx| {
        ctx.dex_file(ch.dex_idx)
            .and_then(|dex| dex.classes.get(ch.class_idx))
            .and_then(|c| c.class_data.as_ref())
            .map(|d| d.virtual_methods.len())
            .unwrap_or(0)
    });
    (0..vm_count)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, true)))
        .collect()
}

#[export]
pub fn class_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, true, true),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn class_static_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, true, false),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn class_instance_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, false, true),
            None => Vec::new(),
        }
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
    class: &stitch_apk::stitch_dex::ClassDef,
    statics: bool,
    instances: bool,
) -> Vec<FieldInfo> {
    let mut fields = Vec::new();
    if let Some(data) = &class.class_data {
        if statics {
            for (i, f) in data.static_fields.iter().enumerate() {
                let field_id = &dex.fields[f.field.0 as usize];
                let initial_value = class
                    .static_values
                    .get(i)
                    .and_then(|v| encoded_val_from_dex(v, dex));
                fields.push(FieldInfo {
                    class_descriptor: dex.type_descriptor(class.class_type).to_string(),
                    name: dex.string(field_id.name).to_string(),
                    field_type: dex.type_descriptor(field_id.type_).to_string(),
                    access_flags: f.access_flags.bits(),
                    initial_value,
                });
            }
        }
        if instances {
            for f in &data.instance_fields {
                let field_id = &dex.fields[f.field.0 as usize];
                fields.push(FieldInfo {
                    class_descriptor: dex.type_descriptor(class.class_type).to_string(),
                    name: dex.string(field_id.name).to_string(),
                    field_type: dex.type_descriptor(field_id.type_).to_string(),
                    access_flags: f.access_flags.bits(),
                    initial_value: None,
                });
            }
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
