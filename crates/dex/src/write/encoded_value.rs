// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::write_uleb128;
use crate::types::encoded_value::{EncodedAnnotation, EncodedValue};

pub fn write_encoded_value(buf: &mut Vec<u8>, value: &EncodedValue) {
    match value {
        EncodedValue::Byte(v) => {
            buf.push(0x00); // type=0x00, arg=0
            buf.push(*v as u8);
        }
        EncodedValue::Short(v) => {
            let bytes = write_signed_int_bytes(*v as i64);
            buf.push(0x02 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Char(v) => {
            let bytes = write_unsigned_int_bytes(*v as u64);
            buf.push(0x03 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Int(v) => {
            let bytes = write_signed_int_bytes(*v as i64);
            buf.push(0x04 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Long(v) => {
            let bytes = write_signed_int_bytes(*v);
            buf.push(0x06 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Float(v) => {
            let raw = v.to_le_bytes();
            let bytes = strip_right_zeros_float(&raw);
            buf.push(0x10 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Double(v) => {
            let raw = v.to_le_bytes();
            let bytes = strip_right_zeros_float(&raw);
            buf.push(0x11 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::MethodType(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x15 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::MethodHandle(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x16 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::String(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x17 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Type(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x18 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Field(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x19 | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Method(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x1a | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Enum(idx) => {
            let bytes = write_unsigned_int_bytes(idx.0 as u64);
            buf.push(0x1b | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(&bytes);
        }
        EncodedValue::Array(values) => {
            buf.push(0x1c);
            write_encoded_array(buf, values);
        }
        EncodedValue::Annotation(ann) => {
            buf.push(0x1d);
            write_encoded_annotation(buf, ann);
        }
        EncodedValue::Null => {
            buf.push(0x1e);
        }
        EncodedValue::Boolean(v) => {
            buf.push(0x1f | ((*v as u8) << 5));
        }
    }
}

pub fn write_encoded_array(buf: &mut Vec<u8>, values: &[EncodedValue]) {
    write_uleb128(buf, values.len() as u32);
    for val in values {
        write_encoded_value(buf, val);
    }
}

pub fn write_encoded_annotation(buf: &mut Vec<u8>, ann: &EncodedAnnotation) {
    write_uleb128(buf, ann.type_.0);
    write_uleb128(buf, ann.elements.len() as u32);
    for elem in &ann.elements {
        write_uleb128(buf, elem.name.0);
        write_encoded_value(buf, &elem.value);
    }
}

fn write_signed_int_bytes(value: i64) -> Vec<u8> {
    let raw = value.to_le_bytes();
    // Find minimal encoding (sign-extended)
    let mut size = 8;
    while size > 1 {
        // Check if we can drop the last byte
        let byte = raw[size - 1];
        let prev_sign = (raw[size - 2] & 0x80) != 0;
        if (byte == 0xFF && prev_sign) || (byte == 0x00 && !prev_sign) {
            size -= 1;
        } else {
            break;
        }
    }
    raw[..size].to_vec()
}

fn write_unsigned_int_bytes(value: u64) -> Vec<u8> {
    let raw = value.to_le_bytes();
    let mut size = 8;
    while size > 1 && raw[size - 1] == 0 {
        size -= 1;
    }
    raw[..size].to_vec()
}

/// For floats/doubles: strip trailing zero bytes from the LEFT (low-order in LE)
/// because float encoding is right-zero-extended.
fn strip_right_zeros_float(raw: &[u8]) -> Vec<u8> {
    let mut start = 0;
    while start < raw.len() - 1 && raw[start] == 0 {
        start += 1;
    }
    raw[start..].to_vec()
}
