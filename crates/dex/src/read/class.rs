// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::annotation::read_annotations_directory;
use super::code::read_code_item;
use super::encoded_value::read_encoded_array_with_opts;
use super::ids::read_type_list;
use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::Result;
use crate::file::RawClassDef;
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassData, ClassDef, EncodedField, EncodedMethod};
use crate::types::header::ParseOptions;
use crate::types::{FieldIdx, MethodIdx};

/// Decodes one `class_def_item` record into a resident [`ClassDef`].
pub fn read_class_def(buf: &[u8], raw: RawClassDef, opts: &ParseOptions) -> Result<ClassDef> {
    let header = raw.header();
    let interfaces = if raw.interfaces_off != 0 {
        read_type_list(buf, raw.interfaces_off)?
    } else {
        crate::types::TypeList::new()
    };
    let annotations = if raw.annotations_off != 0 && opts.include_annotations {
        Some(Box::new(read_annotations_directory(
            buf,
            raw.annotations_off,
            opts,
        )?))
    } else {
        None
    };
    let class_data = if raw.class_data_off != 0 {
        Some(Box::new(read_class_data(buf, raw.class_data_off, opts)?))
    } else {
        None
    };
    let static_values = if raw.static_values_off != 0 {
        read_encoded_array_with_opts(buf, raw.static_values_off as usize, opts)?.0
    } else {
        Vec::new()
    };
    Ok(ClassDef {
        class_type: header.class_type,
        access_flags: header.access_flags,
        superclass: header.superclass,
        interfaces,
        source_file: header.source_file,
        annotations,
        class_data,
        static_values,
    })
}

pub fn read_class_data(buf: &[u8], off: u32, opts: &ParseOptions) -> Result<ClassData> {
    let mut pos = off as usize;

    let (static_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (instance_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (direct_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (virtual_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;

    let (static_fields, new_pos) = read_encoded_fields(buf, pos, static_fields_size, opts)?;
    pos = new_pos;
    let (instance_fields, new_pos) = read_encoded_fields(buf, pos, instance_fields_size, opts)?;
    pos = new_pos;
    let (direct_methods, new_pos) = read_encoded_methods(buf, pos, direct_methods_size, opts)?;
    pos = new_pos;
    let (virtual_methods, _) = read_encoded_methods(buf, pos, virtual_methods_size, opts)?;

    Ok(ClassData {
        static_fields,
        instance_fields,
        direct_methods,
        virtual_methods,
    })
}

fn read_encoded_fields(
    buf: &[u8],
    mut pos: usize,
    count: u32,
    opts: &ParseOptions,
) -> Result<(Vec<EncodedField>, usize)> {
    let mut fields = Vec::with_capacity(count as usize);
    let mut field_idx: u32 = 0;

    for _ in 0..count {
        let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        field_idx = field_idx.wrapping_add(diff);

        let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;

        fields.push(EncodedField {
            field: FieldIdx(field_idx),
            access_flags: AccessFlags::from_bits_retain(access),
        });
    }

    Ok((fields, pos))
}

/// An `encoded_method` entry before its code item is read.
#[derive(Debug, Clone, Copy)]
pub struct MethodHeader {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub code_off: u32,
}

/// A `class_data_item` with method code left in the buffer: the member lists
/// only, so a class can be emitted one method at a time.
pub struct ClassSkeleton {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<MethodHeader>,
    pub virtual_methods: Vec<MethodHeader>,
}

pub fn read_class_skeleton_at(
    buf: &[u8],
    off: usize,
    opts: &ParseOptions,
) -> Result<ClassSkeleton> {
    let mut pos = off;

    let (static_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (instance_fields_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (direct_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;
    let (virtual_methods_size, n) = read_uleb128_with_opts(buf, pos, opts)?;
    pos += n;

    let (static_fields, pos) = read_encoded_fields(buf, pos, static_fields_size, opts)?;
    let (instance_fields, pos) = read_encoded_fields(buf, pos, instance_fields_size, opts)?;
    let (direct_methods, pos) = read_method_headers(buf, pos, direct_methods_size, opts)?;
    let (virtual_methods, _) = read_method_headers(buf, pos, virtual_methods_size, opts)?;

    Ok(ClassSkeleton {
        static_fields,
        instance_fields,
        direct_methods,
        virtual_methods,
    })
}

fn read_method_headers(
    buf: &[u8],
    mut pos: usize,
    count: u32,
    opts: &ParseOptions,
) -> Result<(Vec<MethodHeader>, usize)> {
    let mut headers = Vec::with_capacity(count as usize);
    let mut method_idx: u32 = 0;

    for _ in 0..count {
        let (diff, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;
        method_idx = method_idx.wrapping_add(diff);

        let (access, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;

        let (code_off, n) = read_uleb128_with_opts(buf, pos, opts)?;
        pos += n;

        headers.push(MethodHeader {
            method: MethodIdx(method_idx),
            access_flags: AccessFlags::from_bits_retain(access),
            code_off,
        });
    }

    Ok((headers, pos))
}

fn read_encoded_methods(
    buf: &[u8],
    pos: usize,
    count: u32,
    opts: &ParseOptions,
) -> Result<(Vec<EncodedMethod>, usize)> {
    let (headers, pos) = read_method_headers(buf, pos, count, opts)?;
    let mut methods = Vec::with_capacity(headers.len());
    for header in headers {
        let code = if header.code_off != 0 {
            Some(read_code_item(buf, header.code_off, opts)?)
        } else {
            None
        };
        methods.push(EncodedMethod {
            method: header.method,
            access_flags: header.access_flags,
            code,
        });
    }
    Ok((methods, pos))
}
