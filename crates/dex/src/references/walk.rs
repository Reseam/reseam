// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::RefSink;
use super::items::{annotations_dir, debug_info, encoded_value, instruction};
use crate::DexFile;
use crate::error::Result;
use crate::instruction::stream::{index_operands, walk_instructions};
use crate::model::class::{ClassDef, ClassHeader};
use crate::model::code::CodeItem;
use crate::model::{Pool, TypeIdx, TypeList};
use crate::options::ParseOptions;
use crate::read::annotation::read_annotations_directory;
use crate::read::class::RawClassDef;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::handlers::{HandlerEvent, walk_handler_list};
use crate::read::debug::read_debug_info;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::read::type_list::read_type_list;
use reseam_bytes::require_len;

/// Whether a class walk also visits the bodies of its methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bodies {
    Visit,
    Skip,
}

/// The references of a `class_def_item`'s own fields.
pub(crate) fn class_header(sink: &mut impl RefSink, header: ClassHeader, interfaces: &[TypeIdx]) {
    sink.add(Pool::Type, header.class_type.0);
    if let Some(superclass) = header.superclass {
        sink.add(Pool::Type, superclass.0);
    }
    for interface in interfaces {
        sink.add(Pool::Type, interface.0);
    }
    if let Some(source_file) = header.source_file {
        sink.add(Pool::String, source_file.0);
    }
}

pub(crate) fn class(
    sink: &mut impl RefSink,
    class: &ClassDef,
    dex: &DexFile,
    bodies: Bodies,
) -> Result<()> {
    class_header(sink, ClassHeader::of(class), &class.interfaces);
    if let Some(annotations) = class
        .annotations
        .as_ref()
        .filter(|metadata| metadata.kept_by(dex.write_options().annotations, dex.raw.as_ref()))
    {
        annotations_dir(sink, annotations.read()?.as_ref());
    }
    let Some(data) = &class.class_data else {
        return Ok(());
    };
    for field in &data.static_fields {
        if let Some(value) = class.static_values.get(&field.field) {
            encoded_value(sink, value);
        }
    }
    for field in data.static_fields.iter().chain(&data.instance_fields) {
        sink.add(Pool::Field, field.field.0);
    }
    for method in data.direct_methods.iter().chain(&data.virtual_methods) {
        sink.add(Pool::Method, method.method.0);
        if let Some(code) = method.code.as_ref().filter(|_| bodies == Bodies::Visit) {
            code_item(sink, code, dex)?;
        }
    }
    Ok(())
}

pub(crate) fn raw_class(
    sink: &mut impl RefSink,
    buf: &[u8],
    raw: &RawClassDef,
    opts: ParseOptions,
    options: crate::options::WriteOptions,
    bodies: Bodies,
) -> Result<()> {
    let interfaces = if raw.interfaces_off == 0 {
        TypeList::new()
    } else {
        read_type_list(buf, raw.interfaces_off as usize)
    };
    class_header(sink, raw.header(), &interfaces);
    if raw.annotations_off != 0 && options.annotations == crate::options::MetadataPolicy::Preserve {
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
            if method.code_off != 0 && bodies == Bodies::Visit {
                raw_code_item(sink, buf, method.code_off as usize, opts, options)?;
            }
        }
    }
    Ok(())
}

fn raw_code_item(
    sink: &mut impl RefSink,
    buf: &[u8],
    base: usize,
    opts: ParseOptions,
    options: crate::options::WriteOptions,
) -> Result<()> {
    require_len(buf, base, 16, "code item")?;
    let word = |at: usize| reseam_bytes::u32_at(buf, at);
    let tries_size = u16::from_le_bytes([buf[base + 6], buf[base + 7]]) as usize;
    let debug_off = word(base + 8);
    let insns_size = word(base + 12) as usize;
    walk_instructions(buf, base + 16, insns_size, |insn| {
        for operand in index_operands(insn.opcode) {
            let at = insn.offset() + operand.at;
            let idx = if matches!(operand.width, crate::instruction::stream::IndexWidth::U32) {
                word(at)
            } else {
                u32::from(u16::from_le_bytes([buf[at], buf[at + 1]]))
            };
            sink.add(operand.pool, idx);
        }
        true
    })?;
    if tries_size > 0 {
        let handlers = base + 16 + insns_size.next_multiple_of(2) * 2 + tries_size * 8;
        walk_handler_list(buf, handlers, opts, |event| {
            if let HandlerEvent::Typed(catch) = event {
                sink.add(Pool::Type, catch.exception_type.0);
            }
            Ok(())
        })?;
    }
    if debug_off != 0 && options.debug_info == crate::options::MetadataPolicy::Preserve {
        debug_info(sink, &read_debug_info(buf, debug_off, opts)?);
    }
    Ok(())
}

pub(crate) fn code_item(sink: &mut impl RefSink, code: &CodeItem, dex: &DexFile) -> Result<()> {
    for insn in &code.instructions {
        instruction(sink, insn);
    }
    for handler in &code.catch_handlers {
        for catch in &handler.typed_catches {
            sink.add(Pool::Type, catch.exception_type.0);
        }
    }
    if let Some(debug) = code
        .debug_info
        .as_ref()
        .filter(|metadata| metadata.kept_by(dex.write_options().debug_info, dex.raw.as_ref()))
    {
        debug_info(sink, debug.read()?.as_ref());
    }
    Ok(())
}
