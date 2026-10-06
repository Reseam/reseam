// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::{Result, invalid_encoded_value_type, malformed, slice};
use crate::read::read_u8;
use crate::types::encoded_value::{
    EncodedAnnotation, EncodedAnnotationElement, EncodedValue, ValueKind,
};
use crate::types::header::ParseOptions;
use crate::types::method_handle::MethodHandleIdx;
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

pub fn read_encoded_value_with_opts(
    buf: &[u8],
    pos: usize,
    opts: ParseOptions,
) -> Result<(EncodedValue, usize)> {
    let header = read_u8(buf, pos, "encoded value")?;
    let kind =
        ValueKind::from_tag(header & 0x1f).ok_or_else(|| invalid_encoded_value_type(header))?;
    let arg = usize::from(header >> 5);
    if arg > kind.max_arg() {
        return Err(malformed(
            "encoded value",
            pos,
            "value width exceeds its type",
        ));
    }
    let offset = pos + 1;
    let size = arg + 1;
    let signed = || read_signed_int(buf, offset, size);
    let unsigned = || read_unsigned_int(buf, offset, size);
    let index = || unsigned().map(|value| value as u32);
    let value = match kind {
        ValueKind::Byte => EncodedValue::Byte(signed()? as i8),
        ValueKind::Short => EncodedValue::Short(signed()? as i16),
        ValueKind::Char => EncodedValue::Char(unsigned()? as u16),
        ValueKind::Int => EncodedValue::Int(signed()? as i32),
        ValueKind::Long => EncodedValue::Long(signed()?),
        ValueKind::Float => {
            let mut bytes = [0; 4];
            bytes[4 - size..].copy_from_slice(slice(buf, offset, size, "encoded value")?);
            EncodedValue::Float(f32::from_le_bytes(bytes))
        }
        ValueKind::Double => {
            let mut bytes = [0; 8];
            bytes[8 - size..].copy_from_slice(slice(buf, offset, size, "encoded value")?);
            EncodedValue::Double(f64::from_le_bytes(bytes))
        }
        ValueKind::MethodType => EncodedValue::MethodType(ProtoIdx(index()?)),
        ValueKind::MethodHandle => EncodedValue::MethodHandle(MethodHandleIdx(index()?)),
        ValueKind::String => EncodedValue::String(StringIdx(index()?)),
        ValueKind::Type => EncodedValue::Type(TypeIdx(index()?)),
        ValueKind::Field => EncodedValue::Field(FieldIdx(index()?)),
        ValueKind::Method => EncodedValue::Method(MethodIdx(index()?)),
        ValueKind::Enum => EncodedValue::Enum(FieldIdx(index()?)),
        ValueKind::Array => {
            let (values, consumed) = read_encoded_array_with_opts(buf, offset, opts)?;
            return Ok((EncodedValue::Array(values), consumed + 1));
        }
        ValueKind::Annotation => {
            let (annotation, consumed) = read_encoded_annotation_with_opts(buf, offset, opts)?;
            return Ok((EncodedValue::Annotation(annotation), consumed + 1));
        }
        ValueKind::Null => return Ok((EncodedValue::Null, 1)),
        ValueKind::Boolean => return Ok((EncodedValue::Boolean(arg != 0), 1)),
    };
    Ok((value, size + 1))
}

pub fn read_encoded_array_with_opts(
    buf: &[u8],
    pos: usize,
    opts: ParseOptions,
) -> Result<(Vec<EncodedValue>, usize)> {
    let (size, mut consumed) = read_uleb128_with_opts(buf, pos, opts)?;
    let mut values =
        Vec::with_capacity((size as usize).min(buf.len().saturating_sub(pos + consumed)));
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
    opts: ParseOptions,
) -> Result<(EncodedAnnotation, usize)> {
    let (type_idx, mut consumed) = read_uleb128_with_opts(buf, pos, opts)?;
    let (size, n) = read_uleb128_with_opts(buf, pos + consumed, opts)?;
    consumed += n;

    let mut elements =
        Vec::with_capacity((size as usize).min(buf.len().saturating_sub(pos + consumed) / 2));
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

fn read_signed_int(buf: &[u8], pos: usize, size: usize) -> Result<i64> {
    let bytes = slice(buf, pos, size, "encoded value")?;
    let mut result: i64 = 0;
    for (index, byte) in bytes.iter().copied().enumerate() {
        result |= i64::from(byte) << (index * 8);
    }
    let shift = (size * 8) as u32;
    if shift < 64 {
        let sign_bit = 1i64 << (shift - 1);
        result = (result ^ sign_bit) - sign_bit;
    }
    Ok(result)
}

fn read_unsigned_int(buf: &[u8], pos: usize, size: usize) -> Result<u64> {
    let bytes = slice(buf, pos, size, "encoded value")?;
    let mut result: u64 = 0;
    for (index, byte) in bytes.iter().copied().enumerate() {
        result |= u64::from(byte) << (index * 8);
    }
    Ok(result)
}
