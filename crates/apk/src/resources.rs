use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::error::{invalid, malformed, Result};

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
const MAX_STRING_POOL_STRINGS: usize = 1_000_000;
const MAX_TYPE_ENTRIES: usize = 1_000_000;
const MAX_UTF16_CODE_UNITS: usize = 1_000_000;

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
                return Err(malformed(
                    "resource chunk",
                    pos,
                    "chunk extends past end of table",
                ));
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
                                let res_id =
                                    (pkg.id << 24) | ((res_type.id as u32) << 16) | (i as u32);
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

    pub fn contains_resource_id(&self, res_id: u32) -> bool {
        let pkg_id = (res_id >> 24) & 0xFF;
        let type_id = ((res_id >> 16) & 0xFF) as u8;
        let entry_idx = (res_id & 0xFFFF) as usize;

        self.packages.iter().any(|pkg| {
            pkg.id == pkg_id
                && pkg.types.iter().any(|res_type| {
                    res_type.id == type_id
                        && matches!(res_type.entries.get(entry_idx), Some(Some(_)))
                })
        })
    }

    /// Add a new string to the global string pool and return its index.
    pub fn add_global_string(&mut self, value: &str) -> u32 {
        // Check if already exists
        if let Some(idx) = self.global_strings.iter().position(|s| s == value) {
            return idx as u32;
        }
        let idx = self.global_strings.len() as u32;
        self.global_strings.push(value.to_string());
        idx
    }

    pub fn add_resource(
        &mut self,
        type_name: &str,
        entry_name: &str,
        data_type: u8,
        data: u32,
    ) -> Option<u32> {
        let pkg = self.packages.first_mut()?;
        let type_id = pkg.ensure_type(type_name)?;

        let key_idx = if let Some(idx) = pkg.key_strings.iter().position(|k| k == entry_name) {
            idx as u32
        } else {
            let idx = pkg.key_strings.len() as u32;
            pkg.key_strings.push(entry_name.to_string());
            idx
        };

        let default_type = pkg
            .types
            .iter_mut()
            .find(|t| t.id == type_id && is_default_config(&t.config));

        let entry_index = if let Some(res_type) = default_type {
            if let Some(existing) = res_type
                .entries
                .iter()
                .position(|e| e.as_ref().map_or(false, |e| e.key == key_idx))
            {
                if let Some(Some(entry)) = res_type.entries.get_mut(existing) {
                    entry.value = ResValue::Simple { data_type, data };
                }
                existing
            } else {
                let idx = res_type.entries.len();
                res_type.entries.push(Some(ResEntry {
                    flags: 0,
                    key: key_idx,
                    value: ResValue::Simple { data_type, data },
                }));
                idx
            }
        } else {
            return None;
        };

        if let Some(spec) = pkg.type_specs.iter_mut().find(|s| s.id == type_id) {
            while spec.flags.len() <= entry_index {
                spec.flags.push(0);
            }
        }

        for res_type in &mut pkg.types {
            if res_type.id == type_id && !is_default_config(&res_type.config) {
                while res_type.entries.len() <= entry_index {
                    res_type.entries.push(None);
                }
            }
        }

        let res_id = (pkg.id << 24) | ((type_id as u32) << 16) | (entry_index as u32);
        Some(res_id)
    }

    pub fn add_string_resource(&mut self, name: &str, value: &str) -> Option<u32> {
        let string_idx = self.add_global_string(value);
        self.add_resource("string", name, VALUE_TYPE_STRING, string_idx)
    }

    pub fn add_bool_resource(&mut self, name: &str, value: bool) -> Option<u32> {
        self.add_resource("bool", name, 0x12, if value { 0xFFFF_FFFF } else { 0 })
    }

    pub fn add_integer_resource(&mut self, name: &str, value: i32) -> Option<u32> {
        self.add_resource("integer", name, 0x10, value as u32)
    }

    pub fn add_color_resource(&mut self, name: &str, argb: u32) -> Option<u32> {
        self.add_resource("color", name, 0x1c, argb)
    }

    pub fn add_dimen_resource(&mut self, name: &str, encoded_dim: u32) -> Option<u32> {
        self.add_resource("dimen", name, 0x05, encoded_dim)
    }

    pub fn ensure_id(&mut self, name: &str) -> Option<u32> {
        if let Some(existing) = self.find_resource_id("id", name) {
            return Some(existing);
        }
        self.add_resource("id", name, 0x01, 0)
    }

    pub fn resource_exists(&self, type_name: &str, entry_name: &str) -> bool {
        self.find_entry(type_name, entry_name).is_some()
    }

    pub fn get_string_value(&self, name: &str) -> Option<&str> {
        let (_, _, _, entry) = self.find_entry("string", name)?;
        match &entry.value {
            ResValue::Simple { data_type, data } if *data_type == VALUE_TYPE_STRING => {
                self.get_string(*data)
            }
            _ => None,
        }
    }

    pub fn set_string_value(&mut self, name: &str, value: &str) -> bool {
        let string_idx = match self.find_entry("string", name) {
            Some((_, _, _, entry)) => match &entry.value {
                ResValue::Simple { data_type, data } if *data_type == VALUE_TYPE_STRING => *data,
                _ => return false,
            },
            None => return false,
        };
        self.set_string(string_idx, value.to_string());
        true
    }

    pub fn add_color_parsed(&mut self, name: &str, color: &str) -> Option<u32> {
        let (data_type, data) = crate::axml::compiler::parse_color(color)?;
        self.add_resource("color", name, data_type, data)
    }

    pub fn add_dimen_parsed(&mut self, name: &str, dimen: &str) -> Option<u32> {
        let encoded = crate::axml::compiler::parse_dimension(dimen)?;
        self.add_resource("dimen", name, 0x05, encoded)
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.find_entry(type_name, entry_name)
            .map(|(pkg, res_type, i, _)| (pkg.id << 24) | ((res_type.id as u32) << 16) | (i as u32))
    }

    pub fn get_resource_value(&self, type_name: &str, entry_name: &str) -> Option<(u8, u32)> {
        let (_, _, _, entry) = self.find_entry(type_name, entry_name)?;
        match &entry.value {
            ResValue::Simple { data_type, data } => Some((*data_type, *data)),
            ResValue::Complex { .. } => None,
        }
    }

    fn find_entry(
        &self,
        type_name: &str,
        entry_name: &str,
    ) -> Option<(&ResPackage, &ResType, usize, &ResEntry)> {
        for pkg in &self.packages {
            let Some(type_id) = pkg
                .type_strings
                .iter()
                .position(|t| t == type_name)
                .map(|i| (i + 1) as u8)
            else {
                continue;
            };

            for res_type in &pkg.types {
                if res_type.id != type_id {
                    continue;
                }
                for (i, entry) in res_type.entries.iter().enumerate() {
                    if let Some(entry) = entry {
                        let key_name = pkg.key_strings.get(entry.key as usize);
                        if key_name.map(|k| k.as_str()) == Some(entry_name) {
                            return Some((pkg, res_type, i, entry));
                        }
                    }
                }
            }
        }
        None
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

impl ResPackage {
    pub fn ensure_type(&mut self, type_name: &str) -> Option<u8> {
        if let Some(pos) = self.type_strings.iter().position(|t| t == type_name) {
            return u8::try_from(pos + 1).ok();
        }
        if self.type_strings.len() >= u8::MAX as usize {
            return None;
        }
        self.type_strings.push(type_name.to_string());
        let type_id = self.type_strings.len() as u8;
        self.type_specs.push(TypeSpec {
            id: type_id,
            flags: Vec::new(),
        });
        self.types.push(ResType {
            id: type_id,
            config: ResConfig::default(),
            entries: Vec::new(),
        });
        Some(type_id)
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
    if string_count > MAX_STRING_POOL_STRINGS {
        return Err(invalid(
            "res string pool",
            "string count exceeds safety limit",
        ));
    }
    let _style_count = read_u32_le(data, 12, "res string pool")?;
    let flags = read_u32_le(data, 16, "res string pool")?;
    let strings_start = read_u32_le(data, 20, "res string pool")? as usize;

    let is_utf8 = (flags & (1 << 8)) != 0;
    let offsets_start = header_size;

    let mut strings = Vec::with_capacity(string_count);
    for i in 0..string_count {
        let offset_pos = offsets_start + i * 4;
        if offset_pos + 4 > data.len() {
            return Err(malformed(
                "res string pool",
                offset_pos,
                "string offset table extends past pool",
            ));
        }
        let offset = read_u32_le(data, offset_pos, "res string offset")? as usize;
        let abs = strings_start + offset;

        if abs >= data.len() {
            return Err(malformed(
                "res string pool",
                abs,
                "string offset extends past pool",
            ));
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

// Uses MUTF-8 (Modified UTF-8) decoding because resources.arsc string pools
// encode supplementary characters as surrogate pairs, unlike AXML which uses standard UTF-8.
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
    if char_count > MAX_UTF16_CODE_UNITS {
        return Err(invalid(
            "res utf16 string",
            "string length exceeds safety limit",
        ));
    }

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
            return Err(malformed(
                "package chunk",
                pos,
                "chunk extends past end of package",
            ));
        }

        match ct {
            RES_TABLE_TYPE_SPEC => {
                if hs >= 8 && pos + hs <= data.len() {
                    let type_id = data.get(pos + 8).copied().unwrap_or(0);
                    let entry_count = read_u32_le(data, pos + 12, "type spec")? as usize;
                    if entry_count > MAX_TYPE_ENTRIES {
                        return Err(invalid("type spec", "entry count exceeds safety limit"));
                    }
                    let mut flags = Vec::with_capacity(entry_count);
                    for i in 0..entry_count {
                        let fpos = pos + hs + i * 4;
                        if fpos + 4 <= pos + cs {
                            flags.push(read_u32_le(data, fpos, "type spec flags")?);
                        }
                    }
                    type_specs.push(TypeSpec { id: type_id, flags });
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
    if entry_count > MAX_TYPE_ENTRIES {
        return Err(invalid("res type", "entry count exceeds safety limit"));
    }
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
        encode_utf8(&mut string_data, s);
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

fn is_default_config(config: &ResConfig) -> bool {
    config.data.iter().all(|&b| b == 0)
}

use crate::buf::{write_u16, write_u32};
use crate::string_encoding::encode_utf8;
