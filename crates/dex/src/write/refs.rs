// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reports every pool index a class uses directly, from its IR or straight
//! from the file. Indices that entries of other pools pull in (a method's
//! proto, a type's descriptor) are left to the sink.

use super::raw_code::walk_handlers;
use crate::error::{require_len, Result};
use crate::file::{read_type_list, RawClassDef};
use crate::read::annotation::read_annotations_directory;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::{index_operands, walk_instructions};
use crate::read::debug::read_debug_info;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::ClassDef;
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::header::ParseOptions;
use crate::types::instruction::Instruction;
use crate::types::method_handle::CallSiteItem;
use crate::types::Pool;

pub(crate) trait RefSink {
    fn add(&mut self, pool: Pool, idx: u32);
}

impl<F: FnMut(Pool, u32)> RefSink for F {
    fn add(&mut self, pool: Pool, idx: u32) {
        self(pool, idx)
    }
}

pub(crate) fn class(sink: &mut impl RefSink, class: &ClassDef) {
    sink.add(Pool::Type, class.class_type.0);
    if let Some(superclass) = class.superclass {
        sink.add(Pool::Type, superclass.0);
    }
    for interface in &class.interfaces {
        sink.add(Pool::Type, interface.0);
    }
    if let Some(source_file) = class.source_file {
        sink.add(Pool::String, source_file.0);
    }
    if let Some(annotations) = &class.annotations {
        annotations_dir(sink, annotations);
    }
    for value in &class.static_values {
        encoded_value(sink, value);
    }
    let Some(data) = &class.class_data else {
        return;
    };
    for field in data.static_fields.iter().chain(&data.instance_fields) {
        sink.add(Pool::Field, field.field.0);
    }
    for method in data.direct_methods.iter().chain(&data.virtual_methods) {
        sink.add(Pool::Method, method.method.0);
        if let Some(code) = &method.code {
            code_item(sink, code);
        }
    }
}

/// The same indices [`class`] reports, read from the file record without
/// decoding any code. Annotations are skipped when the file was parsed
/// without them, matching what the writer emits.
pub(crate) fn raw_class(
    sink: &mut impl RefSink,
    buf: &[u8],
    raw: &RawClassDef,
    opts: &ParseOptions,
) -> Result<()> {
    let header = raw.header();
    sink.add(Pool::Type, header.class_type.0);
    if let Some(superclass) = header.superclass {
        sink.add(Pool::Type, superclass.0);
    }
    if let Some(source_file) = header.source_file {
        sink.add(Pool::String, source_file.0);
    }
    if raw.interfaces_off != 0 {
        for interface in read_type_list(buf, raw.interfaces_off as usize) {
            sink.add(Pool::Type, interface.0);
        }
    }
    if raw.annotations_off != 0 && opts.include_annotations {
        annotations_dir(
            sink,
            &read_annotations_directory(buf, raw.annotations_off, opts)?,
        );
    }
    if raw.static_values_off != 0 {
        let (values, _) = read_encoded_array_with_opts(buf, raw.static_values_off as usize, opts)?;
        for value in &values {
            encoded_value(sink, value);
        }
    }
    if raw.class_data_off != 0 {
        let skeleton = read_class_skeleton_at(buf, raw.class_data_off as usize, opts)?;
        for field in skeleton
            .static_fields
            .iter()
            .chain(&skeleton.instance_fields)
        {
            sink.add(Pool::Field, field.field.0);
        }
        for method in skeleton
            .direct_methods
            .iter()
            .chain(&skeleton.virtual_methods)
        {
            sink.add(Pool::Method, method.method.0);
            if method.code_off != 0 {
                raw_code_item(sink, buf, method.code_off as usize, opts)?;
            }
        }
    }
    Ok(())
}

fn raw_code_item(
    sink: &mut impl RefSink,
    buf: &[u8],
    base: usize,
    opts: &ParseOptions,
) -> Result<()> {
    require_len(buf, base, 16, "code item")?;
    let word = |at: usize| u32::from_le_bytes(buf[at..at + 4].try_into().unwrap());
    let tries_size = u16::from_le_bytes([buf[base + 6], buf[base + 7]]) as usize;
    let debug_off = word(base + 8);
    let insns_size = word(base + 12) as usize;
    walk_instructions(buf, base + 16, insns_size, |insn| {
        for operand in index_operands(insn.opcode) {
            let at = insn.offset() + operand.at;
            let idx = if operand.wide {
                word(at)
            } else {
                u16::from_le_bytes([buf[at], buf[at + 1]]) as u32
            };
            sink.add(operand.pool, idx);
        }
        true
    })?;
    if tries_size > 0 {
        let handlers = base + 16 + insns_size.next_multiple_of(2) * 2 + tries_size * 8;
        walk_handlers(buf, handlers, opts, |type_idx| {
            sink.add(Pool::Type, type_idx)
        })?;
    }
    if debug_off != 0 && opts.include_debug_info {
        debug_info(sink, &read_debug_info(buf, debug_off, opts)?);
    }
    Ok(())
}

pub(crate) fn call_site(sink: &mut impl RefSink, call_site: &CallSiteItem) {
    sink.add(Pool::MethodHandle, call_site.bootstrap_method.0);
    sink.add(Pool::String, call_site.method_name.0);
    sink.add(Pool::Proto, call_site.method_type.0 as u32);
    for arg in &call_site.extra_arguments {
        encoded_value(sink, arg);
    }
}

fn code_item(sink: &mut impl RefSink, code: &CodeItem) {
    for insn in &code.instructions {
        instruction(sink, insn);
    }
    for handler in &code.catch_handlers {
        for catch in &handler.typed_catches {
            sink.add(Pool::Type, catch.exception_type.0);
        }
    }
    if let Some(debug) = &code.debug_info {
        debug_info(sink, debug);
    }
}

fn instruction(sink: &mut impl RefSink, insn: &Instruction) {
    if let Some(idx) = insn.string_ref() {
        sink.add(Pool::String, idx.0);
    }
    if let Some(idx) = insn.type_ref() {
        sink.add(Pool::Type, idx.0);
    }
    if let Some(idx) = insn.field_ref() {
        sink.add(Pool::Field, idx.0);
    }
    if let Some(idx) = insn.method_ref() {
        sink.add(Pool::Method, idx.0);
    }
    match insn {
        Instruction::InvokePolymorphic { proto, .. }
        | Instruction::InvokePolymorphicRange { proto, .. }
        | Instruction::ConstMethodType { proto, .. } => sink.add(Pool::Proto, proto.0 as u32),
        Instruction::InvokeCustom { call_site, .. }
        | Instruction::InvokeCustomRange { call_site, .. } => {
            sink.add(Pool::CallSite, call_site.0);
        }
        Instruction::ConstMethodHandle { method_handle, .. } => {
            sink.add(Pool::MethodHandle, method_handle.0);
        }
        _ => {}
    }
}

fn debug_info(sink: &mut impl RefSink, debug: &DebugInfo) {
    for name in debug.parameter_names.iter().flatten() {
        sink.add(Pool::String, name.0);
    }
    for bytecode in &debug.bytecodes {
        let (name, type_, signature) = match *bytecode {
            DebugBytecode::StartLocal { name, type_, .. } => (name, type_, None),
            DebugBytecode::StartLocalExtended {
                name,
                type_,
                signature,
                ..
            } => (name, type_, signature),
            DebugBytecode::SetFile { name } => (name, None, None),
            _ => continue,
        };
        for string in name.into_iter().chain(signature) {
            sink.add(Pool::String, string.0);
        }
        if let Some(type_) = type_ {
            sink.add(Pool::Type, type_.0);
        }
    }
}

fn annotations_dir(sink: &mut impl RefSink, dir: &AnnotationsDirectory) {
    for item in &dir.class_annotations {
        annotation_item(sink, item);
    }
    for (field, items) in &dir.field_annotations {
        sink.add(Pool::Field, field.0);
        items.iter().for_each(|item| annotation_item(sink, item));
    }
    for (method, items) in &dir.method_annotations {
        sink.add(Pool::Method, method.0);
        items.iter().for_each(|item| annotation_item(sink, item));
    }
    for (method, params) in &dir.parameter_annotations {
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

fn encoded_value(sink: &mut impl RefSink, value: &EncodedValue) {
    match value {
        EncodedValue::String(idx) => sink.add(Pool::String, idx.0),
        EncodedValue::Type(idx) => sink.add(Pool::Type, idx.0),
        EncodedValue::Field(idx) | EncodedValue::Enum(idx) => sink.add(Pool::Field, idx.0),
        EncodedValue::Method(idx) => sink.add(Pool::Method, idx.0),
        EncodedValue::MethodType(idx) => sink.add(Pool::Proto, idx.0 as u32),
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
