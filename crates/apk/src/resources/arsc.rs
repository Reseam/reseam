use crate::error::{invalid, malformed, read_u16_le, read_u32_le, require_len, Result};

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    pub global_strings: Vec<String>,
    pub packages: Vec<ResPackage>,
}

#[derive(Debug, Clone)]
pub struct ResPackage {
    pub id: u32,
    pub name: String,
    pub type_strings: Vec<String>,
    pub key_strings: Vec<String>,
    pub type_specs: Vec<TypeSpec>,
    pub types: Vec<ResType>,
}

#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub id: u8,
    pub flags: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ResType {
    pub id: u8,
    pub config: ResConfig,
    pub entries: Vec<Option<ResEntry>>,
}

#[derive(Debug, Clone, Default)]
pub struct ResConfig {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ResEntry {
    pub flags: u16,
    pub key: u32,
    pub value: ResValue,
}

#[derive(Debug, Clone)]
pub enum ResValue {
    Simple { data_type: u8, data: u32 },
    Complex { parent: u32, entries: Vec<MapEntry> },
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub name: u32,
    pub data_type: u8,
    pub data: u32,
}

const VALUE_TYPE_STRING: u8 = 0x03;

impl ResourceTable {
    pub fn parse(data: &[u8]) -> Result<Self> {
        require_len(data, 0, 12, "resource table")?;

        let chunk_type = read_u16_le(data, 0, "resource table")?;
        if chunk_type != RES_TABLE_TYPE {
            return Err(invalid(
                "resource table",
                format!("expected 0x0002, got 0x{chunk_type:04x}"),
            ));
        }

        let header_size = read_u16_le(data, 2, "resource table")? as usize;
        let _total_size = read_u32_le(data, 4, "resource table")?;
        let _package_count = read_u32_le(data, 8, "resource table")?;

        let mut global_strings = Vec::new();
        let mut packages = Vec::new();
        let mut pos = header_size;

        while pos + 8 <= data.len() {
            let ct = read_u16_le(data, pos, "resource chunk")?;
            let hs = read_u16_le(data, pos + 2, "resource chunk")? as usize;
            let cs = read_u32_le(data, pos + 4, "resource chunk")? as usize;

            if cs < 8 || pos + cs > data.len() {
                break;
            }

            match ct {
                RES_STRING_POOL_TYPE if global_strings.is_empty() => {
                    global_strings = parse_res_string_pool(&data[pos..pos + cs])?;
                }
                RES_TABLE_PACKAGE_TYPE => {
                    packages.push(parse_package(&data[pos..pos + cs], hs)?);
                }
                _ => {}
            }

            pos += cs;
        }

        Ok(ResourceTable {
            global_strings,
            packages,
        })
    }

    pub fn get_string(&self, index: u32) -> Option<&str> {
        self.global_strings.get(index as usize).map(|s| s.as_str())
    }

    pub fn set_string(&mut self, index: u32, value: String) {
        if let Some(entry) = self.global_strings.get_mut(index as usize) {
            *entry = value;
        }
    }

    /// Find all string-type entries that reference the global string at `string_index`.
    pub fn find_entries_by_string(&self, string_index: u32) -> Vec<ResourceRef> {
        let mut refs = Vec::new();
        for pkg in &self.packages {
            for res_type in &pkg.types {
                for (i, entry) in res_type.entries.iter().enumerate() {
                    if let Some(entry) = entry {
                        if let ResValue::Simple { data_type, data } = &entry.value {
                            if *data_type == VALUE_TYPE_STRING && *data == string_index {
                                let res_id = (pkg.id << 24)
                                    | ((res_type.id as u32) << 16)
                                    | (i as u32);
                                refs.push(ResourceRef {
                                    res_id,
                                    package_id: pkg.id,
                                    type_id: res_type.id,
                                    entry_index: i as u32,
                                    key_name: pkg
                                        .key_strings
                                        .get(entry.key as usize)
                                        .cloned()
                                        .unwrap_or_default(),
                                });
                            }
                        }
                    }
                }
            }
        }
        refs
    }

    /// Replace a string-type entry's value to point at a different global string index.
    pub fn replace_entry_string(&mut self, res_id: u32, new_string_index: u32) {
        let pkg_id = (res_id >> 24) & 0xFF;
        let type_id = ((res_id >> 16) & 0xFF) as u8;
        let entry_idx = (res_id & 0xFFFF) as usize;

        for pkg in &mut self.packages {
            if pkg.id != pkg_id {
                continue;
            }
            for res_type in &mut pkg.types {
                if res_type.id != type_id {
                    continue;
                }
                if let Some(Some(entry)) = res_type.entries.get_mut(entry_idx) {
                    if let ResValue::Simple { data_type, data } = &mut entry.value {
                        if *data_type == VALUE_TYPE_STRING {
                            *data = new_string_index;
                        }
                    }
                }
            }
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let string_pool_chunk = serialize_res_string_pool(&self.global_strings);
        let mut package_chunks = Vec::new();
        for pkg in &self.packages {
            package_chunks.extend_from_slice(&serialize_package(pkg)?);
        }

        let inner_len = string_pool_chunk.len() + package_chunks.len();
        let total_size = 12 + inner_len;

        let mut out = Vec::with_capacity(total_size);
        write_u16(&mut out, RES_TABLE_TYPE);
        write_u16(&mut out, 12);
        write_u32(&mut out, total_size as u32);
        write_u32(&mut out, self.packages.len() as u32);
        out.extend_from_slice(&string_pool_chunk);
        out.extend_from_slice(&package_chunks);

        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub res_id: u32,
    pub package_id: u32,
    pub type_id: u8,
    pub entry_index: u32,
    pub key_name: String,
}

fn parse_res_string_pool(data: &[u8]) -> Result<Vec<String>> {
    require_len(data, 0, 28, "res string pool")?;

    let header_size = read_u16_le(data, 2, "res string pool")? as usize;
    let string_count = read_u32_le(data, 8, "res string pool")? as usize;
    let _style_count = read_u32_le(data, 12, "res string pool")?;
    let flags = read_u32_le(data, 16, "res string pool")?;
    let strings_start = read_u32_le(data, 20, "res string pool")? as usize;

    let is_utf8 = (flags & (1 << 8)) != 0;
    let offsets_start = header_size;

    let mut strings = Vec::with_capacity(string_count);
    for i in 0..string_count {
        let offset_pos = offsets_start + i * 4;
        if offset_pos + 4 > data.len() {
            break;
        }
        let offset = read_u32_le(data, offset_pos, "res string offset")? as usize;
        let abs = strings_start + offset;

        if abs >= data.len() {
            strings.push(String::new());
            continue;
        }

        let s = if is_utf8 {
            decode_res_utf8(data, abs)?
        } else {
            decode_res_utf16(data, abs)?
        };
        strings.push(s);
    }

    Ok(strings)
}

fn decode_res_utf8(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;
    if pos >= data.len() {
        return Ok(String::new());
    }

    // Skip char length varint
    if data[pos] & 0x80 != 0 {
        pos += 2;
    } else {
        pos += 1;
    }
    if pos >= data.len() {
        return Ok(String::new());
    }

    let byte_len = if data[pos] & 0x80 != 0 {
        let hi = (data[pos] & 0x7F) as usize;
        pos += 1;
        if pos >= data.len() {
            return Ok(String::new());
        }
        let lo = data[pos] as usize;
        pos += 1;
        (hi << 8) | lo
    } else {
        let len = data[pos] as usize;
        pos += 1;
        len
    };

    if pos + byte_len > data.len() {
        return Err(malformed(
            "res utf8 string",
            pos,
            "string extends past pool",
        ));
    }

    stitch_dex::encoding::mutf8::decode_mutf8(&data[pos..pos + byte_len])
        .map_err(|_| invalid("res utf8 string", "invalid UTF-8/MUTF-8"))
}

fn decode_res_utf16(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;
    if pos + 2 > data.len() {
        return Ok(String::new());
    }

    let first = read_u16_le(data, pos, "res utf16 length")?;
    let char_count = if first & 0x8000 != 0 {
        let next = read_u16_le(data, pos + 2, "res utf16 length")? as usize;
        pos += 4;
        (((first & 0x7FFF) as usize) << 16) | next
    } else {
        pos += 2;
        first as usize
    };

    if pos + char_count * 2 > data.len() {
        return Err(malformed(
            "res utf16 string",
            pos,
            "string extends past pool",
        ));
    }

    let mut units = Vec::with_capacity(char_count);
    for i in 0..char_count {
        units.push(read_u16_le(data, pos + i * 2, "res utf16 string")?);
    }

    String::from_utf16(&units).map_err(|_| invalid("res utf16 string", "invalid UTF-16"))
}

fn parse_package(data: &[u8], header_size: usize) -> Result<ResPackage> {
    require_len(data, 0, header_size.max(32), "resource package")?;

    let id = read_u32_le(data, 8, "resource package")?;

    // Package name: 128 u16 code units at offset 12
    let name = {
        let name_start = 12;
        let name_end = (name_start + 256).min(data.len());
        let mut units = Vec::new();
        let mut p = name_start;
        while p + 2 <= name_end {
            let cu = read_u16_le(data, p, "package name")?;
            if cu == 0 {
                break;
            }
            units.push(cu);
            p += 2;
        }
        String::from_utf16(&units).unwrap_or_default()
    };

    let type_strings_offset = read_u32_le(data, 268, "resource package")? as usize;
    let _last_public_type = read_u32_le(data, 272, "resource package")?;
    let key_strings_offset = read_u32_le(data, 276, "resource package")? as usize;

    let type_strings = if type_strings_offset > 0 && type_strings_offset < data.len() {
        let end = find_chunk_end(data, type_strings_offset)?;
        parse_res_string_pool(&data[type_strings_offset..end])?
    } else {
        Vec::new()
    };

    let key_strings = if key_strings_offset > 0 && key_strings_offset < data.len() {
        let end = find_chunk_end(data, key_strings_offset)?;
        parse_res_string_pool(&data[key_strings_offset..end])?
    } else {
        Vec::new()
    };

    let mut type_specs = Vec::new();
    let mut types = Vec::new();

    let body_start = if key_strings_offset > 0 {
        let end = find_chunk_end(data, key_strings_offset)?;
        end
    } else if type_strings_offset > 0 {
        let end = find_chunk_end(data, type_strings_offset)?;
        end
    } else {
        header_size
    };

    let mut pos = body_start;
    while pos + 8 <= data.len() {
        let ct = read_u16_le(data, pos, "package chunk")?;
        let hs = read_u16_le(data, pos + 2, "package chunk")? as usize;
        let cs = read_u32_le(data, pos + 4, "package chunk")? as usize;

        if cs < 8 || pos + cs > data.len() {
            break;
        }

        match ct {
            RES_TABLE_TYPE_SPEC => {
                if hs >= 8 && pos + hs <= data.len() {
                    let type_id = data.get(pos + 8).copied().unwrap_or(0);
                    let entry_count =
                        read_u32_le(data, pos + 12, "type spec")? as usize;
                    let mut flags = Vec::with_capacity(entry_count);
                    for i in 0..entry_count {
                        let fpos = pos + hs + i * 4;
                        if fpos + 4 <= pos + cs {
                            flags.push(read_u32_le(data, fpos, "type spec flags")?);
                        }
                    }
                    type_specs.push(TypeSpec {
                        id: type_id,
                        flags,
                    });
                }
            }
            RES_TABLE_TYPE_TYPE => {
                if let Ok(t) = parse_res_type(&data[pos..pos + cs], hs) {
                    types.push(t);
                }
            }
            _ => {}
        }

        pos += cs;
    }

    Ok(ResPackage {
        id,
        name,
        type_strings,
        key_strings,
        type_specs,
        types,
    })
}

fn parse_res_type(data: &[u8], header_size: usize) -> Result<ResType> {
    require_len(data, 0, header_size.max(20), "res type")?;

    let id = data.get(8).copied().unwrap_or(0);
    let entry_count = read_u32_le(data, 12, "res type")? as usize;
    let entries_start = read_u32_le(data, 16, "res type")? as usize;

    let config_start = 20;
    let config_end = header_size.min(data.len());
    let config = ResConfig {
        data: data[config_start..config_end].to_vec(),
    };

    let offset_table_start = header_size;
    let mut entries = Vec::with_capacity(entry_count);

    for i in 0..entry_count {
        let off_pos = offset_table_start + i * 4;
        if off_pos + 4 > data.len() {
            entries.push(None);
            continue;
        }

        let offset = read_u32_le(data, off_pos, "res type offset")?;
        if offset == 0xFFFF_FFFF {
            entries.push(None);
            continue;
        }

        let entry_pos = entries_start + offset as usize;
        if entry_pos + 8 > data.len() {
            entries.push(None);
            continue;
        }

        let _entry_size = read_u16_le(data, entry_pos, "res entry")?;
        let entry_flags = read_u16_le(data, entry_pos + 2, "res entry")?;
        let key = read_u32_le(data, entry_pos + 4, "res entry")?;

        let is_complex = (entry_flags & 0x0001) != 0;

        let value = if is_complex {
            let parent = if entry_pos + 12 <= data.len() {
                read_u32_le(data, entry_pos + 8, "res entry parent")?
            } else {
                0
            };
            let count = if entry_pos + 16 <= data.len() {
                read_u32_le(data, entry_pos + 12, "res entry count")? as usize
            } else {
                0
            };
            let mut map_entries = Vec::with_capacity(count);
            for j in 0..count {
                let me_pos = entry_pos + 16 + j * 12;
                if me_pos + 12 > data.len() {
                    break;
                }
                let name = read_u32_le(data, me_pos, "map entry")?;
                let dt = data.get(me_pos + 7).copied().unwrap_or(0);
                let d = read_u32_le(data, me_pos + 8, "map entry")?;
                map_entries.push(MapEntry {
                    name,
                    data_type: dt,
                    data: d,
                });
            }
            ResValue::Complex {
                parent,
                entries: map_entries,
            }
        } else if entry_pos + 16 <= data.len() {
            let dt = data.get(entry_pos + 11).copied().unwrap_or(0);
            let d = read_u32_le(data, entry_pos + 12, "res value")?;
            ResValue::Simple {
                data_type: dt,
                data: d,
            }
        } else {
            ResValue::Simple {
                data_type: 0,
                data: 0,
            }
        };

        entries.push(Some(ResEntry {
            flags: entry_flags,
            key,
            value,
        }));
    }

    Ok(ResType {
        id,
        config,
        entries,
    })
}

fn find_chunk_end(data: &[u8], offset: usize) -> Result<usize> {
    if offset + 8 > data.len() {
        return Ok(data.len());
    }
    let cs = read_u32_le(data, offset + 4, "chunk size")? as usize;
    Ok((offset + cs).min(data.len()))
}

fn serialize_res_string_pool(strings: &[String]) -> Vec<u8> {
    let header_size: usize = 28;
    let offsets_size = strings.len() * 4;

    let mut string_data = Vec::new();
    let mut offsets = Vec::with_capacity(strings.len());

    for s in strings {
        offsets.push(string_data.len() as u32);
        // UTF-8 encoding
        let char_len = s.chars().count();
        let byte_len = s.len();

        if char_len > 0x7F {
            string_data.push(((char_len >> 8) & 0x7F) as u8 | 0x80);
            string_data.push((char_len & 0xFF) as u8);
        } else {
            string_data.push(char_len as u8);
        }

        if byte_len > 0x7F {
            string_data.push(((byte_len >> 8) & 0x7F) as u8 | 0x80);
            string_data.push((byte_len & 0xFF) as u8);
        } else {
            string_data.push(byte_len as u8);
        }

        string_data.extend_from_slice(s.as_bytes());
        string_data.push(0);
    }

    let strings_start = header_size + offsets_size;
    let chunk_size = strings_start + string_data.len();
    let padded = (chunk_size + 3) & !3;

    let mut out = Vec::with_capacity(padded);
    write_u16(&mut out, RES_STRING_POOL_TYPE);
    write_u16(&mut out, header_size as u16);
    write_u32(&mut out, padded as u32);
    write_u32(&mut out, strings.len() as u32);
    write_u32(&mut out, 0); // style count
    write_u32(&mut out, 1 << 8); // flags: UTF-8
    write_u32(&mut out, strings_start as u32);
    write_u32(&mut out, 0); // styles start

    for offset in &offsets {
        write_u32(&mut out, *offset);
    }
    out.extend_from_slice(&string_data);

    while out.len() < padded {
        out.push(0);
    }

    out
}

fn serialize_package(pkg: &ResPackage) -> Result<Vec<u8>> {
    let type_pool = serialize_res_string_pool(&pkg.type_strings);
    let key_pool = serialize_res_string_pool(&pkg.key_strings);

    let mut body_chunks = Vec::new();
    for spec in &pkg.type_specs {
        body_chunks.extend_from_slice(&serialize_type_spec(spec));
    }
    for t in &pkg.types {
        body_chunks.extend_from_slice(&serialize_res_type(t)?);
    }

    let header_size: usize = 288;
    let type_strings_offset = header_size;
    let key_strings_offset = type_strings_offset + type_pool.len();
    let total_size = key_strings_offset + key_pool.len() + body_chunks.len();

    let mut out = Vec::with_capacity(total_size);
    write_u16(&mut out, RES_TABLE_PACKAGE_TYPE);
    write_u16(&mut out, header_size as u16);
    write_u32(&mut out, total_size as u32);
    write_u32(&mut out, pkg.id);

    // Package name: 128 u16 code units
    let name_units: Vec<u16> = pkg.name.encode_utf16().collect();
    for i in 0..128 {
        let cu = name_units.get(i).copied().unwrap_or(0);
        write_u16(&mut out, cu);
    }

    write_u32(&mut out, type_strings_offset as u32);
    write_u32(&mut out, 0); // last_public_type
    write_u32(&mut out, key_strings_offset as u32);
    write_u32(&mut out, 0); // last_public_key
    write_u32(&mut out, 0); // type_id_offset

    out.extend_from_slice(&type_pool);
    out.extend_from_slice(&key_pool);
    out.extend_from_slice(&body_chunks);

    Ok(out)
}

fn serialize_type_spec(spec: &TypeSpec) -> Vec<u8> {
    let header_size: u16 = 16;
    let chunk_size = header_size as usize + spec.flags.len() * 4;

    let mut out = Vec::with_capacity(chunk_size);
    write_u16(&mut out, RES_TABLE_TYPE_SPEC);
    write_u16(&mut out, header_size);
    write_u32(&mut out, chunk_size as u32);
    out.push(spec.id);
    out.push(0); // res0
    write_u16(&mut out, 0); // res1
    write_u32(&mut out, spec.flags.len() as u32);

    for flag in &spec.flags {
        write_u32(&mut out, *flag);
    }

    out
}

fn serialize_res_type(t: &ResType) -> Result<Vec<u8>> {
    let config_size = t.config.data.len() + 4; // +4 for the config_size field itself
    let header_size = 20 + config_size;
    let offset_table_size = t.entries.len() * 4;

    let mut entry_data = Vec::new();
    let mut offsets = Vec::with_capacity(t.entries.len());

    for entry in &t.entries {
        match entry {
            None => offsets.push(0xFFFF_FFFFu32),
            Some(e) => {
                offsets.push(entry_data.len() as u32);
                serialize_entry(&mut entry_data, e);
            }
        }
    }

    let entries_start = header_size + offset_table_size;
    let chunk_size = entries_start + entry_data.len();

    let mut out = Vec::with_capacity(chunk_size);
    write_u16(&mut out, RES_TABLE_TYPE_TYPE);
    write_u16(&mut out, header_size as u16);
    write_u32(&mut out, chunk_size as u32);
    out.push(t.id);
    out.push(0); // res0
    write_u16(&mut out, 0); // res1
    write_u32(&mut out, t.entries.len() as u32);
    write_u32(&mut out, entries_start as u32);

    // Config: size + data
    write_u32(&mut out, config_size as u32);
    out.extend_from_slice(&t.config.data);

    for offset in &offsets {
        write_u32(&mut out, *offset);
    }
    out.extend_from_slice(&entry_data);

    Ok(out)
}

fn serialize_entry(out: &mut Vec<u8>, entry: &ResEntry) {
    match &entry.value {
        ResValue::Simple { data_type, data } => {
            write_u16(out, 8); // entry size
            write_u16(out, entry.flags);
            write_u32(out, entry.key);
            write_u16(out, 8); // value size
            out.push(0); // res0
            out.push(*data_type);
            write_u32(out, *data);
        }
        ResValue::Complex { parent, entries } => {
            write_u16(out, 16); // entry size
            write_u16(out, entry.flags | 0x0001);
            write_u32(out, entry.key);
            write_u32(out, *parent);
            write_u32(out, entries.len() as u32);
            for me in entries {
                write_u32(out, me.name);
                write_u16(out, 8); // value size
                out.push(0); // res0
                out.push(me.data_type);
                write_u32(out, me.data);
            }
        }
    }
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
