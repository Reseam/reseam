use crate::axml::reader::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};
use crate::axml::string_pool::StringPool;
use crate::error::Result;

const CHUNK_XML_DOCUMENT: u16 = 0x0003;
const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_RESOURCE_IDS: u16 = 0x0180;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;

const FLAG_UTF8: u32 = 1 << 8;
const TYPE_STRING: u8 = 0x03;
const TYPE_INT_DEC: u8 = 0x10;
const TYPE_INT_HEX: u8 = 0x11;
const TYPE_INT_BOOLEAN: u8 = 0x12;
const TYPE_REFERENCE: u8 = 0x01;

impl AxmlDocument {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let string_pool_chunk = encode_string_pool(&self.string_pool);
        let res_id_chunk = encode_resource_ids(&self.resource_ids);

        let mut event_chunks = Vec::new();
        for event in &self.elements {
            event_chunks.extend_from_slice(&encode_event(event));
        }

        let inner_len = string_pool_chunk.len() + res_id_chunk.len() + event_chunks.len();
        let total_size = 8 + inner_len;

        let mut out = Vec::with_capacity(total_size);
        write_u16(&mut out, CHUNK_XML_DOCUMENT);
        write_u16(&mut out, 8);
        write_u32(&mut out, total_size as u32);
        out.extend_from_slice(&string_pool_chunk);
        out.extend_from_slice(&res_id_chunk);
        out.extend_from_slice(&event_chunks);

        Ok(out)
    }
}

fn encode_string_pool(pool: &StringPool) -> Vec<u8> {
    let string_count = pool.strings.len();
    let header_size: u16 = 28;
    let offsets_size = string_count * 4;

    let mut string_data = Vec::new();
    let mut offsets = Vec::with_capacity(string_count);

    for s in &pool.strings {
        offsets.push(string_data.len() as u32);
        if pool.is_utf8 {
            encode_utf8_string(&mut string_data, s);
        } else {
            encode_utf16_string(&mut string_data, s);
        }
    }

    let strings_start = (header_size as usize) + offsets_size;
    let chunk_size = strings_start + string_data.len();
    let padded_chunk_size = (chunk_size + 3) & !3;

    let mut out = Vec::with_capacity(padded_chunk_size);
    write_u16(&mut out, CHUNK_STRING_POOL);
    write_u16(&mut out, header_size);
    write_u32(&mut out, padded_chunk_size as u32);

    write_u32(&mut out, string_count as u32);
    write_u32(&mut out, 0); // style_count
    let flags = if pool.is_utf8 { FLAG_UTF8 } else { 0 };
    write_u32(&mut out, flags);
    write_u32(&mut out, strings_start as u32);
    write_u32(&mut out, 0); // styles_start

    for offset in &offsets {
        write_u32(&mut out, *offset);
    }

    out.extend_from_slice(&string_data);

    // Pad
    while out.len() < padded_chunk_size {
        out.push(0);
    }

    out
}

fn encode_utf8_string(out: &mut Vec<u8>, s: &str) {
    let char_len = s.chars().count();
    let byte_len = s.len();

    if char_len > 0x7F {
        out.push(((char_len >> 8) & 0x7F) as u8 | 0x80);
        out.push((char_len & 0xFF) as u8);
    } else {
        out.push(char_len as u8);
    }

    if byte_len > 0x7F {
        out.push(((byte_len >> 8) & 0x7F) as u8 | 0x80);
        out.push((byte_len & 0xFF) as u8);
    } else {
        out.push(byte_len as u8);
    }

    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn encode_utf16_string(out: &mut Vec<u8>, s: &str) {
    let code_units: Vec<u16> = s.encode_utf16().collect();
    let char_count = code_units.len();

    if char_count > 0x7FFF {
        out.extend_from_slice(&(((char_count >> 16) as u16) | 0x8000).to_le_bytes());
        out.extend_from_slice(&((char_count & 0xFFFF) as u16).to_le_bytes());
    } else {
        out.extend_from_slice(&(char_count as u16).to_le_bytes());
    }

    for cu in &code_units {
        out.extend_from_slice(&cu.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
}

fn encode_resource_ids(ids: &[u32]) -> Vec<u8> {
    if ids.is_empty() {
        return Vec::new();
    }

    let header_size: u16 = 8;
    let chunk_size = 8 + ids.len() * 4;

    let mut out = Vec::with_capacity(chunk_size);
    write_u16(&mut out, CHUNK_RESOURCE_IDS);
    write_u16(&mut out, header_size);
    write_u32(&mut out, chunk_size as u32);

    for id in ids {
        write_u32(&mut out, *id);
    }

    out
}

fn encode_event(event: &AxmlEvent) -> Vec<u8> {
    match event {
        AxmlEvent::StartNamespace { prefix, uri } => {
            let header_size: u16 = 16;
            let chunk_size: u32 = header_size as u32 + 8;
            let mut out = Vec::with_capacity(chunk_size as usize);
            write_u16(&mut out, CHUNK_START_NAMESPACE);
            write_u16(&mut out, header_size);
            write_u32(&mut out, chunk_size);
            write_u32(&mut out, 0); // line number
            write_u32(&mut out, 0xFFFF_FFFF); // comment
            write_u32(&mut out, prefix.unwrap_or(0xFFFF_FFFF));
            write_u32(&mut out, *uri);
            out
        }
        AxmlEvent::EndNamespace { prefix, uri } => {
            let header_size: u16 = 16;
            let chunk_size: u32 = header_size as u32 + 8;
            let mut out = Vec::with_capacity(chunk_size as usize);
            write_u16(&mut out, CHUNK_END_NAMESPACE);
            write_u16(&mut out, header_size);
            write_u32(&mut out, chunk_size);
            write_u32(&mut out, 0);
            write_u32(&mut out, 0xFFFF_FFFF);
            write_u32(&mut out, prefix.unwrap_or(0xFFFF_FFFF));
            write_u32(&mut out, *uri);
            out
        }
        AxmlEvent::StartElement {
            namespace,
            name,
            attributes,
        } => {
            let header_size: u16 = 16;
            let attr_size: u16 = 20;
            let body_size = 20 + attributes.len() * attr_size as usize;
            let chunk_size = header_size as usize + body_size;
            let mut out = Vec::with_capacity(chunk_size);
            write_u16(&mut out, CHUNK_START_ELEMENT);
            write_u16(&mut out, header_size);
            write_u32(&mut out, chunk_size as u32);
            write_u32(&mut out, 0); // line number
            write_u32(&mut out, 0xFFFF_FFFF); // comment
            write_u32(&mut out, namespace.unwrap_or(0xFFFF_FFFF));
            write_u32(&mut out, *name);
            write_u16(&mut out, 0x14); // attr_start (20 bytes from start of body)
            write_u16(&mut out, attr_size);
            write_u16(&mut out, attributes.len() as u16);
            write_u16(&mut out, 0); // id_index
            write_u16(&mut out, 0); // class_index
            write_u16(&mut out, 0); // style_index

            for attr in attributes {
                encode_attribute(&mut out, attr);
            }

            out
        }
        AxmlEvent::EndElement { namespace, name } => {
            let header_size: u16 = 16;
            let chunk_size: u32 = header_size as u32 + 8;
            let mut out = Vec::with_capacity(chunk_size as usize);
            write_u16(&mut out, CHUNK_END_ELEMENT);
            write_u16(&mut out, header_size);
            write_u32(&mut out, chunk_size);
            write_u32(&mut out, 0);
            write_u32(&mut out, 0xFFFF_FFFF);
            write_u32(&mut out, namespace.unwrap_or(0xFFFF_FFFF));
            write_u32(&mut out, *name);
            out
        }
    }
}

fn encode_attribute(out: &mut Vec<u8>, attr: &AxmlAttribute) {
    write_u32(out, attr.namespace.unwrap_or(0xFFFF_FFFF));
    write_u32(out, attr.name);
    write_u32(out, attr.raw_value.unwrap_or(0xFFFF_FFFF));
    write_u16(out, 8); // typed_value size
    out.push(0); // res0
    let (tv_type, tv_data) = encode_typed_value(&attr.typed_value);
    out.push(tv_type);
    write_u32(out, tv_data);
}

fn encode_typed_value(tv: &TypedValue) -> (u8, u32) {
    match tv {
        TypedValue::String(idx) => (TYPE_STRING, *idx),
        TypedValue::Int(v) => (TYPE_INT_DEC, *v as u32),
        TypedValue::Bool(b) => (TYPE_INT_BOOLEAN, if *b { 0xFFFF_FFFF } else { 0 }),
        TypedValue::Reference(v) => (TYPE_REFERENCE, *v),
        TypedValue::Hex(v) => (TYPE_INT_HEX, *v),
        TypedValue::Other { data_type, data } => (*data_type, *data),
    }
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
