// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DexFile;
use crate::error::{Result, require_len};
use crate::file::{RawClassDef, read_type_list};
use crate::read::annotation::read_annotations_directory;
use crate::read::class::read_class_skeleton_at;
use crate::read::code::payload::{HandlerEvent, walk_handler_list};
use crate::read::code::{index_operands, walk_instructions};
use crate::read::debug::read_debug_info;
use crate::read::encoded_value::read_encoded_array_with_opts;
use crate::types::Pool;
use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::ClassDef;
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::header::ParseOptions;
use crate::types::instruction::Instruction;
use crate::types::method_handle::CallSiteItem;

pub(crate) trait RefSink {
    fn add(&mut self, pool: Pool, idx: u32);
}

impl<F: FnMut(Pool, u32)> RefSink for F {
    fn add(&mut self, pool: Pool, idx: u32) {
        self(pool, idx);
    }
}

pub(crate) fn class(sink: &mut impl RefSink, class: &ClassDef, dex: &DexFile) -> Result<()> {
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
    if let Some(annotations) = class.annotations.as_ref().filter(|metadata| {
        dex.write_options()
            .annotations
            .keeps(metadata, dex.raw.as_ref())
    }) {
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
        if let Some(code) = &method.code {
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
    options: crate::write::WriteOptions,
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
    if raw.annotations_off != 0 && options.annotations == crate::write::MetadataPolicy::Preserve {
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
    options: crate::write::WriteOptions,
) -> Result<()> {
    require_len(buf, base, 16, "code item")?;
    let word = |at: usize| crate::read::u32_at(buf, at);
    let tries_size = u16::from_le_bytes([buf[base + 6], buf[base + 7]]) as usize;
    let debug_off = word(base + 8);
    let insns_size = word(base + 12) as usize;
    walk_instructions(buf, base + 16, insns_size, |insn| {
        for operand in index_operands(insn.opcode) {
            let at = insn.offset() + operand.at;
            let idx = if matches!(operand.width, crate::read::code::refs::IndexWidth::U32) {
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
    if debug_off != 0 && options.debug_info == crate::write::MetadataPolicy::Preserve {
        debug_info(sink, &read_debug_info(buf, debug_off, opts)?);
    }
    Ok(())
}

pub(crate) fn call_site(sink: &mut impl RefSink, call_site: &CallSiteItem) {
    sink.add(Pool::MethodHandle, call_site.bootstrap_method.0);
    sink.add(Pool::String, call_site.method_name.0);
    sink.add(Pool::Proto, call_site.method_type.0);
    for arg in &call_site.extra_arguments {
        encoded_value(sink, arg);
    }
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
    if let Some(debug) = code.debug_info.as_ref().filter(|metadata| {
        dex.write_options()
            .debug_info
            .keeps(metadata, dex.raw.as_ref())
    }) {
        debug_info(sink, debug.read()?.as_ref());
    }
    Ok(())
}

pub(crate) fn instruction(sink: &mut impl RefSink, instruction: &Instruction) {
    for (pool, index) in instruction.indices() {
        sink.add(pool, index);
    }
}

pub(crate) fn debug_info(sink: &mut impl RefSink, debug: &DebugInfo) {
    for name in debug.parameter_names.iter().flatten() {
        sink.add(Pool::String, name.0);
    }
    for bytecode in &debug.bytecodes {
        let (name, type_) = match *bytecode {
            DebugBytecode::StartLocal { name, type_, .. } => (name, type_),
            DebugBytecode::StartLocalExtended {
                name,
                type_,
                signature,
                ..
            } => {
                if let Some(signature) = signature {
                    sink.add(Pool::String, signature.0);
                }
                (name, type_)
            }
            DebugBytecode::SetFile { name } => (name, None),
            _ => continue,
        };
        if let Some(string) = name {
            sink.add(Pool::String, string.0);
        }
        if let Some(type_) = type_ {
            sink.add(Pool::Type, type_.0);
        }
    }
}

pub(crate) fn annotations_dir(sink: &mut impl RefSink, dir: &AnnotationsDirectory) {
    for item in &dir.class {
        annotation_item(sink, item);
    }
    for (field, items) in &dir.fields {
        sink.add(Pool::Field, field.0);
        for item in items {
            annotation_item(sink, item);
        }
    }
    for (method, items) in &dir.methods {
        sink.add(Pool::Method, method.0);
        for item in items {
            annotation_item(sink, item);
        }
    }
    for (method, params) in &dir.parameters {
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

pub(crate) fn encoded_value(sink: &mut impl RefSink, value: &EncodedValue) {
    match value {
        EncodedValue::String(idx) => sink.add(Pool::String, idx.0),
        EncodedValue::Type(idx) => sink.add(Pool::Type, idx.0),
        EncodedValue::Field(idx) | EncodedValue::Enum(idx) => sink.add(Pool::Field, idx.0),
        EncodedValue::Method(idx) => sink.add(Pool::Method, idx.0),
        EncodedValue::MethodType(idx) => sink.add(Pool::Proto, idx.0),
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

pub(crate) struct ReferenceValidator<F> {
    check: F,
    error: Option<crate::DexError>,
}

impl<F: Fn(Pool, u32) -> Result<()>> ReferenceValidator<F> {
    pub(crate) fn new(check: F) -> Self {
        Self { check, error: None }
    }
    pub(crate) fn finish(self) -> Result<()> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl<F: Fn(Pool, u32) -> Result<()>> RefSink for ReferenceValidator<F> {
    fn add(&mut self, pool: Pool, index: u32) {
        if self.error.is_none()
            && let Err(error) = (self.check)(pool, index)
        {
            self.error = Some(error);
        }
    }
}

pub(crate) fn check_index(dex: &DexFile, pool: Pool, index: u32) -> Result<()> {
    let count = pool_len(dex, pool);
    if index as usize >= count {
        return Err(crate::error::invalid(
            "pool reference",
            format!("{pool:?} index {index} exceeds {count} entries"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_pools(dex: &DexFile) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    for descriptor in dex.types.iter() {
        validator.add(Pool::String, descriptor.0);
    }
    for proto in dex.prototypes.iter() {
        validator.add(Pool::String, proto.shorty.0);
        validator.add(Pool::Type, proto.return_type.0);
        for parameter in proto.parameters {
            validator.add(Pool::Type, parameter.0);
        }
    }
    for field in dex.fields.iter() {
        validator.add(Pool::Type, field.class.0);
        validator.add(Pool::Type, field.type_.0);
        validator.add(Pool::String, field.name.0);
    }
    for method in dex.methods.iter() {
        validator.add(Pool::Type, method.class.0);
        validator.add(Pool::Proto, method.proto.0);
        validator.add(Pool::String, method.name.0);
    }
    for handle in dex.method_handles.iter() {
        match handle?.member {
            crate::types::method_handle::MethodHandleMember::Field(field) => {
                validator.add(Pool::Field, field.0);
            }
            crate::types::method_handle::MethodHandleMember::Method(method) => {
                validator.add(Pool::Method, method.0);
            }
        }
    }
    for site in dex.call_sites.iter() {
        call_site(&mut validator, site?.as_ref());
    }
    for index in 0..dex.classes.len() {
        let header = dex.class_header(index);
        validator.add(Pool::Type, header.class_type.0);
        if let Some(superclass) = header.superclass {
            validator.add(Pool::Type, superclass.0);
        }
        if let Some(source) = header.source_file {
            validator.add(Pool::String, source.0);
        }
        if let Some(class) = dex.classes.resident(index) {
            for interface in &class.interfaces {
                validator.add(Pool::Type, interface.0);
            }
        } else if let Some(raw) = dex.classes.raw_def(index) {
            let buffer = dex.raw_buffer().ok_or_else(|| {
                crate::error::invalid("DEX source", "deferred classes require the original buffer")
            })?;
            if raw.interfaces_off != 0 {
                for interface in read_type_list(buffer, raw.interfaces_off as usize) {
                    validator.add(Pool::Type, interface.0);
                }
            }
        }
    }
    validator.finish()
}

pub(crate) fn validate_class(dex: &DexFile, item: &ClassDef) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    class(&mut validator, item, dex)?;
    validator.finish()
}

pub(crate) fn validate_skeleton(dex: &DexFile, skeleton: &crate::ClassSkeleton) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| check_index(dex, pool, index));
    for field in skeleton
        .static_fields
        .iter()
        .chain(&skeleton.instance_fields)
    {
        validator.add(Pool::Field, field.field.0);
    }
    for method in skeleton
        .direct_methods
        .iter()
        .chain(&skeleton.virtual_methods)
    {
        validator.add(Pool::Method, method.method.0);
    }
    validator.finish()
}

pub(crate) fn pool_len(dex: &DexFile, pool: Pool) -> usize {
    match pool {
        Pool::String => dex.strings.len(),
        Pool::Type => dex.types.len(),
        Pool::Proto => dex.prototypes.len(),
        Pool::Field => dex.fields.len(),
        Pool::Method => dex.methods.len(),
        Pool::CallSite => dex.call_sites.len(),
        Pool::MethodHandle => dex.method_handles.len(),
    }
}

pub(crate) fn validate_part(dex: &DexFile, part: &crate::DexPart) -> Result<()> {
    let mut validator = ReferenceValidator::new(|pool, index| {
        check_index(dex, pool, index)?;
        if !part.pools.contains(pool, index) {
            return Err(crate::error::invalid(
                "DEX part",
                format!("{pool:?} index {index} is absent from the selected pools"),
            ));
        }
        Ok(())
    });
    for &index in &part.classes {
        if index >= dex.classes.len() {
            return Err(crate::error::invalid(
                "DEX part",
                "class index exceeds the source class table",
            ));
        }
        if let Some(item) = dex.classes.resident(index) {
            class(&mut validator, item, dex)?;
        } else if let Some(raw) = dex.classes.raw_def(index) {
            raw_class(
                &mut validator,
                dex.raw_buffer().ok_or_else(|| {
                    crate::error::invalid(
                        "DEX source",
                        "deferred classes require the original buffer",
                    )
                })?,
                &raw,
                dex.parse_options,
                dex.write_options(),
            )?;
        }
    }
    validator.finish()
}
