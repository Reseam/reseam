use crate::encoding::leb128::read_uleb128;
use crate::error::{invalid_encoded_value_type, read_u8, Result};
use crate::types::encoded_value::{EncodedAnnotation, EncodedAnnotationElement, EncodedValue};
use crate::types::method_handle::MethodHandleIdx;
use crate::types::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

pub fn read_encoded_value(buf: &[u8], pos: usize) -> Result<(EncodedValue, usize)> {
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
            let v = buf[offset] as i8;
            Ok((EncodedValue::Byte(v), offset + 1 - pos))
        }
        0x02 => {
            // SHORT
            let v = read_signed_int(buf, offset, value_arg + 1) as i16;
            Ok((EncodedValue::Short(v), offset + value_arg + 1 - pos))
        }
        0x03 => {
            // CHAR
            let v = read_unsigned_int(buf, offset, value_arg + 1) as u16;
            Ok((EncodedValue::Char(v), offset + value_arg + 1 - pos))
        }
        0x04 => {
            // INT
            let v = read_signed_int(buf, offset, value_arg + 1) as i32;
            Ok((EncodedValue::Int(v), offset + value_arg + 1 - pos))
        }
        0x06 => {
            // LONG
            let v = read_signed_long(buf, offset, value_arg + 1);
            Ok((EncodedValue::Long(v), offset + value_arg + 1 - pos))
        }
        0x10 => {
            // FLOAT — right-zero-extended
            let mut bytes = [0u8; 4];
            let size = value_arg + 1;
            // Stored bytes go into HIGH bytes of the float (right-zero-extend)
            for i in 0..size {
                bytes[4 - size + i] = buf[offset + i];
            }
            let v = f32::from_le_bytes(bytes);
            Ok((EncodedValue::Float(v), offset + size - pos))
        }
        0x11 => {
            // DOUBLE — right-zero-extended
            let mut bytes = [0u8; 8];
            let size = value_arg + 1;
            for i in 0..size {
                bytes[8 - size + i] = buf[offset + i];
            }
            let v = f64::from_le_bytes(bytes);
            Ok((EncodedValue::Double(v), offset + size - pos))
        }
        0x15 => {
            // METHOD_TYPE
            let v = read_unsigned_int(buf, offset, value_arg + 1) as u16;
            Ok((
                EncodedValue::MethodType(ProtoIdx(v)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x16 => {
            // METHOD_HANDLE
            let v = read_unsigned_int(buf, offset, value_arg + 1);
            Ok((
                EncodedValue::MethodHandle(MethodHandleIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x17 => {
            // STRING
            let v = read_unsigned_int(buf, offset, value_arg + 1);
            Ok((
                EncodedValue::String(StringIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x18 => {
            // TYPE
            let v = read_unsigned_int(buf, offset, value_arg + 1) as u32;
            Ok((EncodedValue::Type(TypeIdx(v)), offset + value_arg + 1 - pos))
        }
        0x19 => {
            // FIELD
            let v = read_unsigned_int(buf, offset, value_arg + 1);
            Ok((
                EncodedValue::Field(FieldIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1a => {
            // METHOD
            let v = read_unsigned_int(buf, offset, value_arg + 1);
            Ok((
                EncodedValue::Method(MethodIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1b => {
            // ENUM
            let v = read_unsigned_int(buf, offset, value_arg + 1);
            Ok((
                EncodedValue::Enum(FieldIdx(v as u32)),
                offset + value_arg + 1 - pos,
            ))
        }
        0x1c => {
            // ARRAY
            let (arr, size) = read_encoded_array(buf, offset)?;
            Ok((EncodedValue::Array(arr), offset + size - pos))
        }
        0x1d => {
            // ANNOTATION
            let (ann, size) = read_encoded_annotation(buf, offset)?;
            Ok((EncodedValue::Annotation(ann), offset + size - pos))
        }
        0x1e => {
            // NULL
            Ok((EncodedValue::Null, 1))
        }
        0x1f => {
            // BOOLEAN — value_arg IS the value
            Ok((EncodedValue::Boolean(value_arg != 0), 1))
        }
        _ => Err(invalid_encoded_value_type(header)),
    }
}

pub fn read_encoded_array(buf: &[u8], pos: usize) -> Result<(Vec<EncodedValue>, usize)> {
    let (size, mut consumed) = read_uleb128(buf, pos)?;
    let mut values = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let (val, n) = read_encoded_value(buf, pos + consumed)?;
        consumed += n;
        values.push(val);
    }
    Ok((values, consumed))
}

pub fn read_encoded_annotation(buf: &[u8], pos: usize) -> Result<(EncodedAnnotation, usize)> {
    let (type_idx, mut consumed) = read_uleb128(buf, pos)?;
    let (size, n) = read_uleb128(buf, pos + consumed)?;
    consumed += n;

    let mut elements = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let (name_idx, n) = read_uleb128(buf, pos + consumed)?;
        consumed += n;
        let (value, n) = read_encoded_value(buf, pos + consumed)?;
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

fn read_signed_int(buf: &[u8], pos: usize, size: usize) -> i64 {
    let mut result: i64 = 0;
    for i in 0..size {
        result |= (buf[pos + i] as i64) << (i * 8);
    }
    // Sign extend
    let shift = (size * 8) as u32;
    if shift < 64 {
        let sign_bit = 1i64 << (shift - 1);
        result = (result ^ sign_bit) - sign_bit;
    }
    result
}

fn read_signed_long(buf: &[u8], pos: usize, size: usize) -> i64 {
    read_signed_int(buf, pos, size)
}

fn read_unsigned_int(buf: &[u8], pos: usize, size: usize) -> u64 {
    let mut result: u64 = 0;
    for i in 0..size {
        result |= (buf[pos + i] as u64) << (i * 8);
    }
    result
}
