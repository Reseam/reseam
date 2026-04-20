// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::{invalid_encoded_value_type, malformed, read_u8, slice, Result};
use crate::types::header::ParseOptions;
use crate::types::encoded_value::{EncodedAnnotation, EncodedAnnotationElement, EncodedValue};
use crate::types::method_handle::MethodHandleIdx;
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

pub fn read_encoded_value_with_opts(
    buf: &[u8],
    pos: usize,
    opts: &ParseOptions,
) -> Result<(EncodedValue, usize)> {
    if pos >= buf.len() {
        return Err(crate::error::buffer_exhausted("encoded value", pos));
    }
    let header = read_u8(buf, pos, "encoded value")?;
    let value_type = header & 0x1F;
    let value_arg = (header >> 5) as usize;
    let offset = pos + 1;

    match value_type {
        0x00 => {
            // BYTE
            validate_encoded_width(pos, "byte", value_arg, 0)?;
            let v = read_fixed_bytes(buf, offset, 1, "encoded value")?[0] as i8;
            Ok((EncodedValue::Byte(v), offset + 1 - pos))
        }
        0x02 => {
            // SHORT
            validate_encoded_width(pos, "short", value_arg, 1)?;
            let v = read_signed_int(buf, offset, value_arg + 1)? as i16;
            Ok((EncodedValue::Short(v), offset + value_arg + 1 - pos))
        }
        0x03 => {
            // CHAR
            validate_encoded_width(pos, "char", value_arg, 1)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)? as u16;
            Ok((EncodedValue::Char(v), offset + value_arg + 1 - pos))
        }
        0x04 => {
            // INT
            validate_encoded_width(pos, "int", value_arg, 3)?;
            let v = read_signed_int(buf, offset, value_arg + 1)? as i32;
            Ok((EncodedValue::Int(v), offset + value_arg + 1 - pos))
        }
        0x06 => {
            // LONG
            validate_encoded_width(pos, "long", value_arg, 7)?;
            let v = read_signed_long(buf, offset, value_arg + 1)?;
            Ok((EncodedValue::Long(v), offset + value_arg + 1 - pos))
        }
        0x10 => {
            // FLOAT — right-zero-extended
            validate_encoded_width(pos, "float", value_arg, 3)?;
            let mut bytes = [0u8; 4];
            let size = value_arg + 1;
            let payload = read_fixed_bytes(buf, offset, size, "encoded value")?;
            // Stored bytes go into HIGH bytes of the float (right-zero-extend)
            for i in 0..size {
                bytes[4 - size + i] = payload[i];
            }
            let v = f32::from_le_bytes(bytes);
            Ok((EncodedValue::Float(v), offset + size - pos))
        }
        0x11 => {
            // DOUBLE — right-zero-extended
            validate_encoded_width(pos, "double", value_arg, 7)?;
            let mut bytes = [0u8; 8];
            let size = value_arg + 1;
            let payload = read_fixed_bytes(buf, offset, size, "encoded value")?;
            for i in 0..size {
                bytes[8 - size + i] = payload[i];
            }
            let v = f64::from_le_bytes(bytes);
            Ok((EncodedValue::Double(v), offset + size - pos))
        }
        0x15 => {
            // METHOD_TYPE
            validate_encoded_width(pos, "method type", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)? as u16;
            Ok((
                EncodedValue::MethodType(ProtoIdx(v)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x16 => {
            // METHOD_HANDLE
            validate_encoded_width(pos, "method handle", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)?;
            Ok((
                EncodedValue::MethodHandle(MethodHandleIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x17 => {
            // STRING
            validate_encoded_width(pos, "string", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)?;
            Ok((
                EncodedValue::String(StringIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x18 => {
            // TYPE
            validate_encoded_width(pos, "type", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)? as u32;
            Ok((EncodedValue::Type(TypeIdx(v)), offset + value_arg + 1 - pos))
        }
        0x19 => {
            // FIELD
            validate_encoded_width(pos, "field", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)?;
            Ok((
                EncodedValue::Field(FieldIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1a => {
            // METHOD
            validate_encoded_width(pos, "method", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)?;
            Ok((
                EncodedValue::Method(MethodIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1b => {
            // ENUM
            validate_encoded_width(pos, "enum", value_arg, 3)?;
            let v = read_unsigned_int(buf, offset, value_arg + 1)?;
            Ok((
                EncodedValue::Enum(FieldIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1c => {
            // ARRAY
            validate_encoded_width(pos, "array", value_arg, 0)?;
            let (arr, size) = read_encoded_array_with_opts(buf, offset, opts)?;
            Ok((EncodedValue::Array(arr), offset + size - pos))
        }
        0x1d => {
            // ANNOTATION
            validate_encoded_width(pos, "annotation", value_arg, 0)?;
            let (ann, size) = read_encoded_annotation_with_opts(buf, offset, opts)?;
            Ok((EncodedValue::Annotation(ann), offset + size - pos))
        }
        0x1e => {
            // NULL
            validate_encoded_width(pos, "null", value_arg, 0)?;
            Ok((EncodedValue::Null, 1))
        }
        0x1f => {
            // BOOLEAN — value_arg IS the value
            validate_encoded_width(pos, "boolean", value_arg, 1)?;
            Ok((EncodedValue::Boolean(value_arg != 0), 1))
        }
        _ => Err(invalid_encoded_value_type(header)),
    }
}

pub fn read_encoded_array_with_opts(
    buf: &[u8],
    pos: usize,
    opts: &ParseOptions,
) -> Result<(Vec<EncodedValue>, usize)> {
    let (size, mut consumed) = read_uleb128_with_opts(buf, pos, opts)?;
    let mut values = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let (val, n) = read_encoded_value_with_opts(buf, pos + consumed, opts)?;
        consumed += n;
        values.push(val);
    }
    Ok((values, consumed))
}

pub fn read_encoded_annotation_with_opts(
    buf: &[u8],
    pos: usize,
    opts: &ParseOptions,
) -> Result<(EncodedAnnotation, usize)> {
    let (type_idx, mut consumed) = read_uleb128_with_opts(buf, pos, opts)?;
    let (size, n) = read_uleb128_with_opts(buf, pos + consumed, opts)?;
    consumed += n;

    let mut elements = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let (name_idx, n) = read_uleb128_with_opts(buf, pos + consumed, opts)?;
        consumed += n;
        let (value, n) = read_encoded_value_with_opts(buf, pos + consumed, opts)?;
        consumed += n;
        elements.push(EncodedAnnotationElement {
            name: StringIdx(name_idx),
            value,
        });
    }

    Ok((
        EncodedAnnotation {
            type_: TypeIdx(type_idx),
            elements,
        },
        consumed,
    ))
}

fn validate_encoded_width(
    pos: usize,
    value_name: &'static str,
    value_arg: usize,
    max_value_arg: usize,
) -> Result<()> {
    if value_arg > max_value_arg {
        return Err(malformed(
            "encoded value",
            pos,
            format!("{value_name} value_arg {value_arg} exceeds maximum {max_value_arg}"),
        ));
    }
    Ok(())
}

fn read_fixed_bytes<'a>(
    buf: &'a [u8],
    pos: usize,
    size: usize,
    section: &'static str,
) -> Result<&'a [u8]> {
    slice(buf, pos, size, section)
}

fn read_signed_int(buf: &[u8], pos: usize, size: usize) -> Result<i64> {
    let bytes = read_fixed_bytes(buf, pos, size, "encoded value")?;
    let mut result: i64 = 0;
    for (index, byte) in bytes.iter().copied().enumerate().take(size) {
        result |= (byte as i64) << (index * 8);
    }
    // Sign extend
    let shift = (size * 8) as u32;
    if shift < 64 {
        let sign_bit = 1i64 << (shift - 1);
        result = (result ^ sign_bit) - sign_bit;
    }
    Ok(result)
}

fn read_signed_long(buf: &[u8], pos: usize, size: usize) -> Result<i64> {
    read_signed_int(buf, pos, size)
}

fn read_unsigned_int(buf: &[u8], pos: usize, size: usize) -> Result<u64> {
    let bytes = read_fixed_bytes(buf, pos, size, "encoded value")?;
    let mut result: u64 = 0;
    for (index, byte) in bytes.iter().copied().enumerate().take(size) {
        result |= (byte as u64) << (index * 8);
    }
    Ok(result)
}
