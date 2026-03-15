use super::header_reader::{u16_at, u32_at};
use crate::encoding::leb128::read_uleb128;
use crate::encoding::mutf8::decode_mutf8;
use crate::error::Result;
use crate::model::field::FieldId;
use crate::model::method::MethodId;
use crate::model::proto::{ProtoIdx, Prototype};
use crate::model::string::{DexString, StringIdx};
use crate::model::types::TypeIdx;

pub fn read_string_ids(buf: &[u8], off: u32, count: u32) -> Result<Vec<u32>> {
    let mut offsets = Vec::with_capacity(count as usize);
    let base = off as usize;
    for i in 0..count as usize {
        offsets.push(u32_at(buf, base + i * 4));
    }
    Ok(offsets)
}

pub fn read_string_data(buf: &[u8], string_data_off: u32) -> Result<DexString> {
    let pos = string_data_off as usize;
    let (_utf16_size, leb_size) = read_uleb128(buf, pos)?;
    let data_start = pos + leb_size;

    // Find null terminator
    let mut end = data_start;
    while end < buf.len() && buf[end] != 0 {
        end += 1;
    }

    let s = decode_mutf8(&buf[data_start..end])?;
    Ok(DexString::new(s))
}

pub fn read_type_ids(buf: &[u8], off: u32, count: u32) -> Result<Vec<StringIdx>> {
    let mut types = Vec::with_capacity(count as usize);
    let base = off as usize;
    for i in 0..count as usize {
        types.push(StringIdx(u32_at(buf, base + i * 4)));
    }
    Ok(types)
}

pub fn read_proto_ids(buf: &[u8], off: u32, count: u32) -> Result<Vec<Prototype>> {
    let mut protos = Vec::with_capacity(count as usize);
    let base = off as usize;
    for i in 0..count as usize {
        let entry_off = base + i * 12;
        let shorty = StringIdx(u32_at(buf, entry_off));
        let return_type = TypeIdx(u32_at(buf, entry_off + 4));
        let params_off = u32_at(buf, entry_off + 8);

        let parameters = if params_off != 0 {
            read_type_list(buf, params_off)?
        } else {
            Vec::new()
        };

        protos.push(Prototype {
            shorty,
            return_type,
            parameters,
        });
    }
    Ok(protos)
}

pub fn read_type_list(buf: &[u8], off: u32) -> Result<Vec<TypeIdx>> {
    let base = off as usize;
    let size = u32_at(buf, base) as usize;
    let mut list = Vec::with_capacity(size);
    for i in 0..size {
        list.push(TypeIdx(u16_at(buf, base + 4 + i * 2) as u32));
    }
    Ok(list)
}

pub fn read_field_ids(buf: &[u8], off: u32, count: u32) -> Result<Vec<FieldId>> {
    let mut fields = Vec::with_capacity(count as usize);
    let base = off as usize;
    for i in 0..count as usize {
        let entry_off = base + i * 8;
        fields.push(FieldId {
            class: TypeIdx(u16_at(buf, entry_off) as u32),
            type_: TypeIdx(u16_at(buf, entry_off + 2) as u32),
            name: StringIdx(u32_at(buf, entry_off + 4)),
        });
    }
    Ok(fields)
}

pub fn read_method_ids(buf: &[u8], off: u32, count: u32) -> Result<Vec<MethodId>> {
    let mut methods = Vec::with_capacity(count as usize);
    let base = off as usize;
    for i in 0..count as usize {
        let entry_off = base + i * 8;
        methods.push(MethodId {
            class: TypeIdx(u16_at(buf, entry_off) as u32),
            proto: ProtoIdx(u16_at(buf, entry_off + 2)),
            name: StringIdx(u32_at(buf, entry_off + 4)),
        });
    }
    Ok(methods)
}
