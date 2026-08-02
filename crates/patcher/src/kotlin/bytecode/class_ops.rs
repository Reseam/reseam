// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use reseam_apk::reseam_dex::{
    AccessFlags, AnnotationElement as DexAnnotationElement, AnnotationItem as DexAnnotationItem,
    AnnotationVisibility as DexAnnotationVisibility, AnnotationsDirectory,
    CatchHandler as DexCatchHandler, CodeItem, DexFile, EncodedField, EncodedMethod, EncodedValue,
    FieldIdx, Instruction as DexInsn, StringIdx, TryItem as DexTryItem,
    TypedCatch as DexTypedCatch,
};

use boltffi::export;

use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::types::{AnnotationItem, EncodedVal, NewField, NewMethod};
use crate::kotlin::{get_method_mut, get_method_ref, with_ctx, with_handles, BUNDLE_DIR};

#[export]
pub fn set_class_access_flags(c: u32, flags: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        dex.classes[ch.class_idx].access_flags = AccessFlags::from_bits_truncate(flags);
    });
}

#[export]
pub fn set_superclass(c: u32, superclass: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let _ = dex.set_superclass(ch.class_idx, &superclass);
    });
}

#[export]
pub fn add_interface(c: u32, interface_descriptor: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let type_idx = dex.intern_type(&interface_descriptor);
        dex.classes[ch.class_idx].interfaces.push(type_idx);
    });
}

#[export]
pub fn remove_class(c: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let class_type = dex.classes[ch.class_idx].class_type;
        dex.remove_class(class_type);
    });
}

#[export]
pub fn create_class(dex_index: u32, descriptor: String, flags: u32, superclass: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(dex_index as usize) {
            Some(d) => d,
            None => return 0,
        };
        let af = AccessFlags::from_bits_truncate(flags);
        match dex.create_class(&descriptor, af, Some(&superclass)) {
            Ok(class_idx) => with_handles(|h| h.alloc_class(dex_index as usize, class_idx)),
            Err(_) => 0,
        }
    })
}

#[export]
pub fn add_method(c: u32, method: NewMethod) -> u32 {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return 0,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return 0,
        };
        let class_desc = dex
            .type_descriptor(dex.classes[ch.class_idx].class_type)
            .to_string();
        let method_idx = match dex.intern_method(&class_desc, &method.name, &method.proto) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = AccessFlags::from_bits_truncate(method.access_flags);
        let code = if af.contains(AccessFlags::NATIVE) || af.contains(AccessFlags::ABSTRACT) {
            None
        } else {
            let insns: Vec<DexInsn> = method
                .instructions
                .iter()
                .map(|ki| kotlin_to_dex(ki, dex))
                .collect();
            let tries: Vec<DexTryItem> = method
                .tries
                .iter()
                .map(|t| DexTryItem {
                    start_addr: t.start_addr,
                    insn_count: t.insn_count,
                    handler_idx: t.handler_idx as usize,
                })
                .collect();
            let catch_handlers: Vec<DexCatchHandler> = method
                .catch_handlers
                .iter()
                .map(|ch| {
                    let typed_catches = ch
                        .typed_catches
                        .iter()
                        .map(|tc| {
                            let type_idx = dex.intern_type(&tc.exception_type);
                            DexTypedCatch {
                                exception_type: type_idx,
                                addr: tc.addr,
                            }
                        })
                        .collect();
                    DexCatchHandler {
                        typed_catches,
                        catch_all_addr: ch.catch_all_addr,
                    }
                })
                .collect();
            Some(CodeItem {
                registers_size: method.registers_size,
                ins_size: method.ins_size,
                outs_size: method.outs_size,
                debug_info: None,
                instructions: insns,
                tries,
                catch_handlers,
            })
        };
        let em = EncodedMethod {
            method: method_idx,
            access_flags: af,
            code,
        };
        let is_virtual = !af.contains(AccessFlags::STATIC)
            && !af.contains(AccessFlags::CONSTRUCTOR)
            && !af.intersects(AccessFlags::PRIVATE);
        let class = &mut dex.classes[ch.class_idx];
        let mi = if is_virtual {
            class.add_virtual_method(em);
            class
                .class_data
                .as_ref()
                .map(|d| d.virtual_methods.len() - 1)
                .unwrap_or(0)
        } else {
            class.add_direct_method(em);
            class
                .class_data
                .as_ref()
                .map(|d| d.direct_methods.len() - 1)
                .unwrap_or(0)
        };
        with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, mi, is_virtual))
    })
}

#[export]
pub fn remove_method(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.class_dex_mut(mh.dex_idx, mh.class_idx) {
            Some(d) => d,
            None => return,
        };
        let class = &mut dex.classes[mh.class_idx];
        if let Some(data) = &mut class.class_data {
            if mh.is_virtual {
                if mh.method_idx < data.virtual_methods.len() {
                    data.virtual_methods.remove(mh.method_idx);
                }
            } else if mh.method_idx < data.direct_methods.len() {
                data.direct_methods.remove(mh.method_idx);
            }
        }
    });
}

#[export]
pub fn set_method_access_flags(m: u32, flags: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.class_dex_mut(mh.dex_idx, mh.class_idx) {
            Some(d) => d,
            None => return,
        };
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
        method.access_flags = AccessFlags::from_bits_truncate(flags);
    });
}

#[export]
pub fn add_field(c: u32, field: NewField) -> u32 {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return 0,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return 0,
        };
        let class_desc = dex
            .type_descriptor(dex.classes[ch.class_idx].class_type)
            .to_string();
        let field_idx = match dex.intern_field(&class_desc, &field.name, &field.field_type) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = AccessFlags::from_bits_truncate(field.access_flags);
        let ef = EncodedField {
            field: field_idx,
            access_flags: af,
        };
        let init_val = field
            .initial_value
            .as_ref()
            .map(|v| encoded_val_to_dex(v, dex));
        let class = &mut dex.classes[ch.class_idx];
        if af.contains(AccessFlags::STATIC) {
            class.add_static_field(ef);
            if let Some(val) = init_val {
                let static_field_count = class
                    .class_data
                    .as_ref()
                    .map(|d| d.static_fields.len())
                    .unwrap_or(0);
                while class.static_values.len() < static_field_count - 1 {
                    class.static_values.push(EncodedValue::Null);
                }
                class.static_values.push(val);
            }
        } else {
            class.add_instance_field(ef);
        }
        0
    })
}

#[export]
pub fn remove_field(c: u32, name: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return,
        };
        let fields_to_remove: Vec<FieldIdx> = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .map(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .filter(|f| dex.string(dex.fields[f.field.0 as usize].name) == name)
                    .map(|f| f.field)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            data.static_fields
                .retain(|f| !fields_to_remove.contains(&f.field));
            data.instance_fields
                .retain(|f| !fields_to_remove.contains(&f.field));
        }
    });
}

#[export]
pub fn set_field_access_flags(c: u32, field_name: String, flags: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return,
        };
        let af = AccessFlags::from_bits_truncate(flags);
        let target_field = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .and_then(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .find(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
                    .map(|f| f.field)
            });
        if let Some(field_idx) = target_field {
            if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
                for f in data
                    .static_fields
                    .iter_mut()
                    .chain(data.instance_fields.iter_mut())
                {
                    if f.field == field_idx {
                        f.access_flags = af;
                        return;
                    }
                }
            }
        }
    });
}

#[export]
pub fn set_static_field_value(c: u32, field_name: String, value: EncodedVal) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return,
        };
        let class = &dex.classes[ch.class_idx];
        let static_idx = class.class_data.as_ref().and_then(|data| {
            data.static_fields
                .iter()
                .position(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
        });
        if let Some(idx) = static_idx {
            let val = encoded_val_to_dex(&value, dex);
            let class = &mut dex.classes[ch.class_idx];
            while class.static_values.len() <= idx {
                class.static_values.push(EncodedValue::Null);
            }
            class.static_values[idx] = val;
        }
    });
}

#[export]
pub fn clone_method(m: u32, new_name: Option<String>) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.class_dex_mut(mh.dex_idx, mh.class_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = match get_method_ref(dex, mh) {
            Some(m) => m.clone(),
            None => return 0,
        };
        let method_idx = if let Some(name) = new_name {
            let mid = &dex.methods[method.method.0 as usize];
            let class_desc = dex.type_descriptor(mid.class).to_string();
            let proto = &dex.prototypes[mid.proto.0 as usize];
            let ret = dex.type_descriptor(proto.return_type);
            let params: Vec<&str> = proto
                .parameters
                .iter()
                .map(|p| dex.type_descriptor(*p))
                .collect();
            let proto_str = format!("({}){}", params.join(""), ret);
            match dex.intern_method(&class_desc, &name, &proto_str) {
                Ok(idx) => idx,
                Err(_) => return 0,
            }
        } else {
            method.method
        };
        let cloned = EncodedMethod {
            method: method_idx,
            access_flags: method.access_flags,
            code: method.code.clone(),
        };
        let class = &mut dex.classes[mh.class_idx];
        let (mi, is_virtual) = if mh.is_virtual {
            class.add_virtual_method(cloned);
            (
                class
                    .class_data
                    .as_ref()
                    .map(|d| d.virtual_methods.len() - 1)
                    .unwrap_or(0),
                true,
            )
        } else {
            class.add_direct_method(cloned);
            (
                class
                    .class_data
                    .as_ref()
                    .map(|d| d.direct_methods.len() - 1)
                    .unwrap_or(0),
                false,
            )
        };
        with_handles(|h| h.alloc_method(mh.dex_idx, mh.class_idx, mi, is_virtual))
    })
}

#[export]
pub fn superclass_chain(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let locations: Vec<usize> = with_ctx(|ctx| {
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut locs = Vec::new();
        let mut current_type = dex.classes[ch.class_idx].superclass;
        while let Some(super_type) = current_type {
            let found = dex
                .classes
                .iter()
                .enumerate()
                .find(|(_, cl)| cl.class_type == super_type);
            match found {
                Some((ci, class)) => {
                    locs.push(ci);
                    current_type = class.superclass;
                }
                None => break,
            }
        }
        locs
    });
    locations
        .into_iter()
        .map(|ci| with_handles(|h| h.alloc_class(ch.dex_idx, ci)))
        .collect()
}

#[export]
pub fn definal_class(c: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return,
        };
        dex.classes[ch.class_idx]
            .access_flags
            .remove(AccessFlags::FINAL);
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            for m in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                m.access_flags.remove(AccessFlags::FINAL);
            }
        }
    });
}

#[export]
pub fn dex_count() -> u32 {
    with_ctx(|ctx| ctx.dex_count() as u32)
}

#[export]
pub fn method_dex(m: u32) -> u32 {
    with_handles(|h| h.get_method(m))
        .map(|mh| mh.dex_idx as u32)
        .unwrap_or(0)
}

#[export]
pub fn intern_string(d: u32, s: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        dex.intern_string(&s).0
    })
}

#[export]
pub fn intern_type(d: u32, descriptor: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        dex.intern_type(&descriptor).0
    })
}

#[export]
pub fn intern_proto(d: u32, proto: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_proto(&proto) {
            Ok(idx) => idx.0 as u32,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn intern_method(d: u32, descriptor: String, name: String, proto: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_method(&descriptor, &name, &proto) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn intern_field(d: u32, descriptor: String, name: String, field_type: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_field(&descriptor, &name, &field_type) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn find_string_idx(d: u32, s: String) -> Option<u32> {
    with_ctx(|ctx| {
        let dex = ctx.dex_file(d as usize)?;
        dex.find_string_idx(&s).map(|idx| idx.0)
    })
}

#[export]
pub fn get_string(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file(d as usize) {
            Some(d) => d,
            None => return String::new(),
        };
        dex.string(StringIdx(idx)).to_string()
    })
}

#[export]
pub fn get_type_descriptor(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file(d as usize) {
            Some(d) => d,
            None => return String::new(),
        };
        use reseam_apk::reseam_dex::TypeIdx;
        dex.type_descriptor(TypeIdx(idx)).to_string()
    })
}

#[export]
pub fn build_lookups(d: u32) {
    with_ctx(|ctx| {
        if let Some(dex) = ctx.dex_file_mut(d as usize) {
            dex.build_lookups();
        }
    });
}

#[export]
pub fn merge_extension_dex(paths: Vec<String>) -> u32 {
    let bundle_dir = BUNDLE_DIR.with(|bd| bd.borrow().clone());
    let bundle_dir = match bundle_dir {
        Some(dir) => dir,
        None => return 0,
    };
    let full_paths: Vec<std::path::PathBuf> = paths.iter().map(|p| bundle_dir.join(p)).collect();
    with_ctx(|ctx| match ctx.merge_extension_dex(&full_paths) {
        Ok(count) => count as u32,
        Err(_) => 0,
    })
}

#[export]
pub fn add_class_annotation(c: u32, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let ann = annotation_to_dex(&annotation, dex);
        let class = &mut dex.classes[ch.class_idx];
        let dir = class
            .annotations
            .get_or_insert_with(|| AnnotationsDirectory {
                class_annotations: Vec::new(),
                field_annotations: Vec::new(),
                method_annotations: Vec::new(),
                parameter_annotations: Vec::new(),
            });
        dir.class_annotations.push(ann);
    });
}

#[export]
pub fn add_method_annotation(m: u32, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.class_dex_mut(mh.dex_idx, mh.class_idx) {
            Some(d) => d,
            None => return,
        };
        let method_idx = match get_method_ref(dex, mh) {
            Some(m) => m.method,
            None => return,
        };
        let ann = annotation_to_dex(&annotation, dex);
        let class = &mut dex.classes[mh.class_idx];
        let dir = class
            .annotations
            .get_or_insert_with(|| AnnotationsDirectory {
                class_annotations: Vec::new(),
                field_annotations: Vec::new(),
                method_annotations: Vec::new(),
                parameter_annotations: Vec::new(),
            });
        if let Some(entry) = dir
            .method_annotations
            .iter_mut()
            .find(|(mid, _)| *mid == method_idx)
        {
            entry.1.push(ann);
        } else {
            dir.method_annotations.push((method_idx, vec![ann]));
        }
    });
}

#[export]
pub fn add_field_annotation(c: u32, field_name: String, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.class_dex_mut(ch.dex_idx, ch.class_idx) {
            Some(d) => d,
            None => return,
        };
        let target_field = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .and_then(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .find(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
                    .map(|f| f.field)
            });
        if let Some(field_idx) = target_field {
            let ann = annotation_to_dex(&annotation, dex);
            let class = &mut dex.classes[ch.class_idx];
            let dir = class
                .annotations
                .get_or_insert_with(|| AnnotationsDirectory {
                    class_annotations: Vec::new(),
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                });
            if let Some(entry) = dir
                .field_annotations
                .iter_mut()
                .find(|(fid, _)| *fid == field_idx)
            {
                entry.1.push(ann);
            } else {
                dir.field_annotations.push((field_idx, vec![ann]));
            }
        }
    });
}

fn encoded_val_to_dex(val: &EncodedVal, dex: &mut DexFile) -> EncodedValue {
    match val {
        EncodedVal::Null => EncodedValue::Null,
        EncodedVal::BoolVal(b) => EncodedValue::Boolean(*b),
        EncodedVal::ByteVal(v) => EncodedValue::Byte(*v),
        EncodedVal::ShortVal(v) => EncodedValue::Short(*v),
        EncodedVal::CharVal(v) => EncodedValue::Char(*v),
        EncodedVal::IntVal(v) => EncodedValue::Int(*v),
        EncodedVal::LongVal(v) => EncodedValue::Long(*v),
        EncodedVal::FloatVal(v) => EncodedValue::Float(*v),
        EncodedVal::DoubleVal(v) => EncodedValue::Double(*v),
        EncodedVal::StringVal(s) => {
            let idx = dex.intern_string(s);
            EncodedValue::String(idx)
        }
        EncodedVal::TypeVal(desc) => {
            let idx = dex.intern_type(desc);
            EncodedValue::Type(idx)
        }
    }
}

fn annotation_to_dex(ann: &AnnotationItem, dex: &mut DexFile) -> DexAnnotationItem {
    let visibility = match ann.visibility {
        0 => DexAnnotationVisibility::Build,
        1 => DexAnnotationVisibility::Runtime,
        2 => DexAnnotationVisibility::System,
        _ => DexAnnotationVisibility::Build,
    };
    let type_idx = dex.intern_type(&ann.annotation_type);
    let elements = ann
        .elements
        .iter()
        .map(|el| {
            let name_idx = dex.intern_string(&el.name);
            let value = encoded_val_to_dex(&el.value, dex);
            DexAnnotationElement {
                name: name_idx,
                value,
            }
        })
        .collect();
    DexAnnotationItem {
        visibility,
        type_: type_idx,
        elements,
    }
}
