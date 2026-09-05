// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Class-level mutation: members, flags, annotations, and DEX pool interning.

use boltffi::export;
use reseam_apk::reseam_dex::{
    AccessFlags, AnnotationElement as DexAnnotationElement, AnnotationItem as DexAnnotationItem,
    AnnotationVisibility, CatchHandler as DexCatchHandler, ClassDef, CodeItem, DexFile,
    EncodedField, EncodedMethod, EncodedValue, FieldIdx, StringIdx, TryItem as DexTryItem, TypeIdx,
    TypedCatch as DexTypedCatch,
};

use crate::context::{ClassLocation, MethodLocation};
use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::handles::{
    alloc_class, alloc_method, bundle_path, class_location, method_location, method_ref,
    with_class_mut, with_ctx, with_method_mut,
};
use crate::kotlin::types::{AnnotationItem, EncodedVal, NewField, NewMethod};

/// A class's DEX with the class left unmaterialized, for header-level edits.
fn with_class_header<R>(
    c: u32,
    f: impl FnOnce(&mut DexFile, ClassLocation) -> Option<R>,
) -> Option<R> {
    let location = class_location(c)?;
    with_ctx(|ctx| f(ctx.dex_file_mut(location.dex_idx)?, location))
}

#[export]
pub fn set_class_access_flags(c: u32, flags: u32) {
    with_class_header(c, |dex, loc| {
        dex.class_mut(loc.class_idx).ok()?.access_flags = AccessFlags::from_bits_truncate(flags);
        Some(())
    });
}

#[export]
pub fn set_superclass(c: u32, superclass: String) {
    with_class_header(c, |dex, loc| {
        dex.set_superclass(loc.class_idx, &superclass).ok()
    });
}

#[export]
pub fn add_interface(c: u32, interface_descriptor: String) {
    with_class_header(c, |dex, loc| {
        let interface = dex.intern_type(&interface_descriptor);
        dex.class_mut(loc.class_idx)
            .ok()?
            .interfaces
            .push(interface);
        Some(())
    });
}

#[export]
pub fn remove_class(c: u32) {
    with_class_header(c, |dex, loc| {
        let class_type = dex.class_header(loc.class_idx).class_type;
        dex.remove_class(class_type).ok()
    });
}

/// A new empty class in DEX `dex_index`; 0 when creation fails.
#[export]
pub fn create_class(dex_index: u32, descriptor: String, flags: u32, superclass: String) -> u32 {
    with_ctx(|ctx| {
        let dex = ctx.dex_file_mut(dex_index as usize)?;
        let class_idx = dex
            .create_class(
                &descriptor,
                AccessFlags::from_bits_truncate(flags),
                Some(&superclass),
            )
            .ok()?;
        Some(alloc_class(ClassLocation {
            dex_idx: dex_index as usize,
            class_idx,
        }))
    })
    .unwrap_or(0)
}

#[export]
pub fn definal_class(c: u32) {
    with_class_mut(c, |dex, loc| {
        dex.class_mut(loc.class_idx).ok()?.definal();
        Some(())
    });
}

#[export]
pub fn superclass_chain(c: u32) -> Vec<u32> {
    let Some(location) = class_location(c) else {
        return Vec::new();
    };
    let chain = with_ctx(|ctx| {
        ctx.dex_file(location.dex_idx)
            .map(|dex| dex.superclass_chain(location.class_idx))
    });
    chain
        .unwrap_or_default()
        .into_iter()
        .map(|class_idx| {
            alloc_class(ClassLocation {
                dex_idx: location.dex_idx,
                class_idx,
            })
        })
        .collect()
}

/// Adds a method; static, constructor and private methods are direct, the
/// rest virtual. Returns its handle, or 0 when the class cannot take it.
#[export]
pub fn add_method(c: u32, method: NewMethod) -> u32 {
    with_class_mut(c, |dex, loc| {
        let class_desc = dex
            .type_descriptor(dex.class_header(loc.class_idx).class_type)
            .into_owned();
        let method_idx = dex
            .intern_method(&class_desc, &method.name, &method.proto)
            .ok()?;
        let flags = AccessFlags::from_bits_truncate(method.access_flags);
        let code = if flags.intersects(AccessFlags::NATIVE | AccessFlags::ABSTRACT) {
            None
        } else {
            Some(CodeItem {
                registers_size: method.registers_size,
                ins_size: method.ins_size,
                outs_size: method.outs_size,
                debug_info: None,
                instructions: method
                    .instructions
                    .iter()
                    .map(|insn| kotlin_to_dex(insn, dex))
                    .collect(),
                tries: method
                    .tries
                    .iter()
                    .map(|t| DexTryItem {
                        start_addr: t.start_addr,
                        insn_count: t.insn_count,
                        handler_idx: t.handler_idx as usize,
                    })
                    .collect(),
                catch_handlers: method
                    .catch_handlers
                    .iter()
                    .map(|handler| DexCatchHandler {
                        typed_catches: handler
                            .typed_catches
                            .iter()
                            .map(|tc| DexTypedCatch {
                                exception_type: dex.intern_type(&tc.exception_type),
                                addr: tc.addr,
                            })
                            .collect(),
                        catch_all_addr: handler.catch_all_addr,
                    })
                    .collect(),
            })
        };
        let is_virtual = !flags
            .intersects(AccessFlags::STATIC | AccessFlags::CONSTRUCTOR | AccessFlags::PRIVATE);
        let encoded = EncodedMethod {
            method: method_idx,
            access_flags: flags,
            code,
        };
        push_method(dex.class_mut(loc.class_idx).ok()?, loc, encoded, is_virtual)
    })
    .unwrap_or(0)
}

/// Adds `method` to `class` and returns the handle of the new slot.
fn push_method(
    class: &mut ClassDef,
    loc: ClassLocation,
    method: EncodedMethod,
    is_virtual: bool,
) -> Option<u32> {
    if is_virtual {
        class.add_virtual_method(method);
    } else {
        class.add_direct_method(method);
    }
    let data = class.class_data.as_ref()?;
    let list = if is_virtual {
        &data.virtual_methods
    } else {
        &data.direct_methods
    };
    Some(alloc_method(MethodLocation {
        dex_idx: loc.dex_idx,
        class_idx: loc.class_idx,
        method_idx: list.len() - 1,
        is_virtual,
    }))
}

#[export]
pub fn remove_method(m: u32) {
    with_method_mut(m, |dex, loc| {
        let data = dex.class_mut(loc.class_idx).ok()?.class_data.as_mut()?;
        let list = if loc.is_virtual {
            &mut data.virtual_methods
        } else {
            &mut data.direct_methods
        };
        (loc.method_idx < list.len()).then(|| list.remove(loc.method_idx))
    });
}

#[export]
pub fn set_method_access_flags(m: u32, flags: u32) {
    with_method_mut(m, |dex, loc| {
        crate::kotlin::handles::method_mut(dex, loc)?.access_flags =
            AccessFlags::from_bits_truncate(flags);
        Some(())
    });
}

/// A copy of the method in the same class, under `new_name` when given.
#[export]
pub fn clone_method(m: u32, new_name: Option<String>) -> u32 {
    with_method_mut(m, |dex, loc| {
        let method = method_ref(dex, loc)?.clone();
        let method_idx = match new_name {
            Some(name) => {
                let id = dex.method_id(method.method);
                let class_desc = dex.type_descriptor(id.class).into_owned();
                let proto = dex.proto_descriptor(&dex.proto(id.proto));
                dex.intern_method(&class_desc, &name, &proto).ok()?
            }
            None => method.method,
        };
        let cloned = EncodedMethod {
            method: method_idx,
            ..method
        };
        let class_loc = ClassLocation {
            dex_idx: loc.dex_idx,
            class_idx: loc.class_idx,
        };
        push_method(
            dex.class_mut(loc.class_idx).ok()?,
            class_loc,
            cloned,
            loc.is_virtual,
        )
    })
    .unwrap_or(0)
}

/// Adds a field; a static field's `initial_value` becomes its static value.
#[export]
pub fn add_field(c: u32, field: NewField) {
    with_class_mut(c, |dex, loc| {
        let class_desc = dex
            .type_descriptor(dex.class_header(loc.class_idx).class_type)
            .into_owned();
        let field_idx = dex
            .intern_field(&class_desc, &field.name, &field.field_type)
            .ok()?;
        let flags = AccessFlags::from_bits_truncate(field.access_flags);
        let initial_value = field.initial_value.as_ref().map(|v| encoded_value(v, dex));
        let encoded = EncodedField {
            field: field_idx,
            access_flags: flags,
        };
        let class = dex.class_mut(loc.class_idx).ok()?;
        if !flags.contains(AccessFlags::STATIC) {
            class.add_instance_field(encoded);
            return Some(());
        }
        class.add_static_field(encoded);
        if let Some(value) = initial_value {
            let slot = class
                .class_data
                .as_ref()
                .map_or(0, |d| d.static_fields.len())
                - 1;
            set_static_value(class, slot, value);
        }
        Some(())
    });
}

fn set_static_value(class: &mut ClassDef, slot: usize, value: EncodedValue) {
    if class.static_values.len() <= slot {
        class.static_values.resize(slot + 1, EncodedValue::Null);
    }
    class.static_values[slot] = value;
}

fn field_named(dex: &DexFile, class_idx: usize, name: &str) -> Option<FieldIdx> {
    let data = dex.resident_class(class_idx)?.class_data.as_ref()?;
    data.static_fields
        .iter()
        .chain(&data.instance_fields)
        .map(|f| f.field)
        .find(|&field| dex.string(dex.field_id(field).name) == name)
}

#[export]
pub fn remove_field(c: u32, name: String) {
    with_class_mut(c, |dex, loc| {
        let field = field_named(dex, loc.class_idx, &name)?;
        let data = dex.class_mut(loc.class_idx).ok()?.class_data.as_mut()?;
        data.static_fields.retain(|f| f.field != field);
        data.instance_fields.retain(|f| f.field != field);
        Some(())
    });
}

#[export]
pub fn set_field_access_flags(c: u32, field_name: String, flags: u32) {
    with_class_mut(c, |dex, loc| {
        let field = field_named(dex, loc.class_idx, &field_name)?;
        let data = dex.class_mut(loc.class_idx).ok()?.class_data.as_mut()?;
        let entry = data
            .static_fields
            .iter_mut()
            .chain(data.instance_fields.iter_mut())
            .find(|f| f.field == field)?;
        entry.access_flags = AccessFlags::from_bits_truncate(flags);
        Some(())
    });
}

#[export]
pub fn set_static_field_value(c: u32, field_name: String, value: EncodedVal) {
    with_class_mut(c, |dex, loc| {
        let slot = dex
            .resident_class(loc.class_idx)?
            .class_data
            .as_ref()?
            .static_fields
            .iter()
            .position(|f| dex.string(dex.field_id(f.field).name) == field_name)?;
        let value = encoded_value(&value, dex);
        set_static_value(dex.class_mut(loc.class_idx).ok()?, slot, value);
        Some(())
    });
}

#[export]
pub fn add_class_annotation(c: u32, annotation: AnnotationItem) {
    with_class_header(c, |dex, loc| {
        let annotation = annotation_item(&annotation, dex);
        let class = dex.class_mut(loc.class_idx).ok()?;
        class
            .annotations
            .get_or_insert_with(Default::default)
            .class_annotations
            .push(annotation);
        Some(())
    });
}

#[export]
pub fn add_method_annotation(m: u32, annotation: AnnotationItem) {
    with_method_mut(m, |dex, loc| {
        let method_idx = method_ref(dex, loc)?.method;
        let annotation = annotation_item(&annotation, dex);
        let directory = dex
            .class_mut(loc.class_idx)
            .ok()?
            .annotations
            .get_or_insert_with(Default::default);
        match directory
            .method_annotations
            .iter_mut()
            .find(|(idx, _)| *idx == method_idx)
        {
            Some((_, list)) => list.push(annotation),
            None => directory
                .method_annotations
                .push((method_idx, vec![annotation])),
        }
        Some(())
    });
}

#[export]
pub fn add_field_annotation(c: u32, field_name: String, annotation: AnnotationItem) {
    with_class_mut(c, |dex, loc| {
        let field = field_named(dex, loc.class_idx, &field_name)?;
        let annotation = annotation_item(&annotation, dex);
        let directory = dex
            .class_mut(loc.class_idx)
            .ok()?
            .annotations
            .get_or_insert_with(Default::default);
        match directory
            .field_annotations
            .iter_mut()
            .find(|(idx, _)| *idx == field)
        {
            Some((_, list)) => list.push(annotation),
            None => directory.field_annotations.push((field, vec![annotation])),
        }
        Some(())
    });
}

#[export]
pub fn dex_count() -> u32 {
    with_ctx(|ctx| ctx.dex().len() as u32)
}

#[export]
pub fn method_dex(m: u32) -> u32 {
    method_location(m).map_or(0, |loc| loc.dex_idx as u32)
}

fn with_dex<R>(d: u32, f: impl FnOnce(&mut DexFile) -> Option<R>) -> Option<R> {
    with_ctx(|ctx| f(ctx.dex_file_mut(d as usize)?))
}

#[export]
pub fn intern_string(d: u32, s: String) -> u32 {
    with_dex(d, |dex| Some(dex.intern_string(&s).0)).unwrap_or(0)
}

#[export]
pub fn intern_type(d: u32, descriptor: String) -> u32 {
    with_dex(d, |dex| Some(dex.intern_type(&descriptor).0)).unwrap_or(0)
}

#[export]
pub fn intern_proto(d: u32, proto: String) -> u32 {
    with_dex(d, |dex| {
        dex.intern_proto(&proto).ok().map(|idx| idx.0 as u32)
    })
    .unwrap_or(0)
}

#[export]
pub fn intern_method(d: u32, descriptor: String, name: String, proto: String) -> u32 {
    with_dex(d, |dex| {
        dex.intern_method(&descriptor, &name, &proto)
            .ok()
            .map(|idx| idx.0)
    })
    .unwrap_or(0)
}

#[export]
pub fn intern_field(d: u32, descriptor: String, name: String, field_type: String) -> u32 {
    with_dex(d, |dex| {
        dex.intern_field(&descriptor, &name, &field_type)
            .ok()
            .map(|idx| idx.0)
    })
    .unwrap_or(0)
}

#[export]
pub fn find_string_idx(d: u32, s: String) -> Option<u32> {
    with_ctx(|ctx| {
        ctx.dex_file(d as usize)?
            .find_string_idx(&s)
            .map(|idx| idx.0)
    })
}

#[export]
pub fn get_string(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        Some(
            ctx.dex_file(d as usize)?
                .string(StringIdx(idx))
                .into_owned(),
        )
    })
    .unwrap_or_default()
}

#[export]
pub fn get_type_descriptor(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        Some(
            ctx.dex_file(d as usize)?
                .type_descriptor(TypeIdx(idx))
                .into_owned(),
        )
    })
    .unwrap_or_default()
}

#[export]
pub fn build_lookups(d: u32) {
    with_dex(d, |dex| {
        dex.build_lookups();
        Some(())
    });
}

/// Merges DEX files from the bundle into the app; returns how many.
#[export]
pub fn merge_extension_dex(paths: Vec<String>) -> u32 {
    let paths: Vec<_> = paths.iter().map(|p| bundle_path(p)).collect();
    with_ctx(|ctx| {
        ctx.merge_extension_dex(&paths)
            .map_or(0, |count| count as u32)
    })
}

fn encoded_value(value: &EncodedVal, dex: &mut DexFile) -> EncodedValue {
    match value {
        EncodedVal::Null => EncodedValue::Null,
        EncodedVal::BoolVal(v) => EncodedValue::Boolean(*v),
        EncodedVal::ByteVal(v) => EncodedValue::Byte(*v),
        EncodedVal::ShortVal(v) => EncodedValue::Short(*v),
        EncodedVal::CharVal(v) => EncodedValue::Char(*v),
        EncodedVal::IntVal(v) => EncodedValue::Int(*v),
        EncodedVal::LongVal(v) => EncodedValue::Long(*v),
        EncodedVal::FloatVal(v) => EncodedValue::Float(*v),
        EncodedVal::DoubleVal(v) => EncodedValue::Double(*v),
        EncodedVal::StringVal(s) => EncodedValue::String(dex.intern_string(s)),
        EncodedVal::TypeVal(desc) => EncodedValue::Type(dex.intern_type(desc)),
    }
}

fn annotation_item(item: &AnnotationItem, dex: &mut DexFile) -> DexAnnotationItem {
    DexAnnotationItem {
        visibility: match item.visibility {
            1 => AnnotationVisibility::Runtime,
            2 => AnnotationVisibility::System,
            _ => AnnotationVisibility::Build,
        },
        type_: dex.intern_type(&item.annotation_type),
        elements: item
            .elements
            .iter()
            .map(|element| DexAnnotationElement {
                name: dex.intern_string(&element.name),
                value: encoded_value(&element.value, dex),
            })
            .collect(),
    }
}
