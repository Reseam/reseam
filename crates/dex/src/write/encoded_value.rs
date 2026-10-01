// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::write_uleb128;
use crate::types::encoded_value::{EncodedAnnotation, EncodedValue};

pub fn write_encoded_value(buf: &mut Vec<u8>, value: &EncodedValue) {
    let tag = value.kind() as u8;
    match value {
        EncodedValue::Byte(v) => {
            buf.push(tag);
            buf.push(*v as u8);
        }
        EncodedValue::Short(v) => {
            let raw = i64::from(*v).to_le_bytes();
            let bytes = signed_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Char(v) => {
            let raw = u64::from(*v).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Int(v) => {
            let raw = i64::from(*v).to_le_bytes();
            let bytes = signed_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Long(v) => {
            let raw = v.to_le_bytes();
            let bytes = signed_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Float(v) => {
            let raw = v.to_le_bytes();
            let bytes = strip_right_zeros_float(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Double(v) => {
            let raw = v.to_le_bytes();
            let bytes = strip_right_zeros_float(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::MethodType(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::MethodHandle(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::String(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Type(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Field(idx) | EncodedValue::Enum(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }
        EncodedValue::Method(idx) => {
            let raw = u64::from(idx.0).to_le_bytes();
            let bytes = unsigned_bytes(&raw);
            buf.push(tag | (((bytes.len() - 1) as u8) << 5));
            buf.extend_from_slice(bytes);
        }

        EncodedValue::Array(values) => {
            buf.push(tag);
            write_encoded_array(buf, values);
        }
        EncodedValue::Annotation(ann) => {
            buf.push(tag);
            write_encoded_annotation(buf, ann);
        }
        EncodedValue::Null => {
            buf.push(tag);
        }
        EncodedValue::Boolean(v) => {
            buf.push(tag | (u8::from(*v) << 5));
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

fn signed_bytes(raw: &[u8; 8]) -> &[u8] {
    let mut size = 8;
    while size > 1 {
        let byte = raw[size - 1];
        let prev_sign = (raw[size - 2] & 0x80) != 0;
        if (byte == 0xFF && prev_sign) || (byte == 0x00 && !prev_sign) {
            size -= 1;
        } else {
            break;
        }
    }
    &raw[..size]
}

fn unsigned_bytes(raw: &[u8; 8]) -> &[u8] {
    let mut size = 8;
    while size > 1 && raw[size - 1] == 0 {
        size -= 1;
    }
    &raw[..size]
}

fn strip_right_zeros_float(raw: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < raw.len() - 1 && raw[start] == 0 {
        start += 1;
    }
    &raw[start..]
}
