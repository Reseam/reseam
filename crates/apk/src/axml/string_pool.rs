// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buf::{read_u16_le, read_u32_le, read_u8};
use crate::error::{invalid, malformed, Result};

const MAX_STRING_POOL_STRINGS: usize = 1_000_000;
const MAX_UTF16_CODE_UNITS: usize = 1_000_000;

/// A parsed AXML string pool.
#[derive(Debug, Clone)]
pub struct StringPool {
    pub strings: Vec<String>,
    pub is_utf8: bool,
}

const FLAG_UTF8: u32 = 1 << 8;

impl StringPool {
    /// Parse a string pool from chunk data (starting after the chunk header type+header_size+size).
    /// `data` should start at the string_count field (offset 8 from chunk start).
    pub fn parse(chunk_data: &[u8], _chunk_start: usize) -> Result<Self> {
        crate::buf::require_len(chunk_data, 0, 20, "axml string pool")?;

        let string_count = read_u32_le(chunk_data, 0, "axml string pool")? as usize;
        if string_count > MAX_STRING_POOL_STRINGS {
            return Err(invalid(
                "axml string pool",
                "string count exceeds safety limit",
            ));
        }
        let _style_count = read_u32_le(chunk_data, 4, "axml string pool")? as usize;
        let flags = read_u32_le(chunk_data, 8, "axml string pool")?;
        let strings_start = read_u32_le(chunk_data, 12, "axml string pool")? as usize;
        let _styles_start = read_u32_le(chunk_data, 16, "axml string pool")? as usize;

        let is_utf8 = (flags & FLAG_UTF8) != 0;

        let offsets_start = 20;
        crate::buf::require_len(
            chunk_data,
            offsets_start,
            string_count.saturating_mul(4),
            "axml string offsets",
        )?;

        let mut strings = Vec::with_capacity(string_count);
        let str_data_offset = strings_start.saturating_sub(8);

        for i in 0..string_count {
            let offset =
                read_u32_le(chunk_data, offsets_start + i * 4, "axml string offsets")? as usize;
            let abs_offset = str_data_offset + offset;

            if abs_offset >= chunk_data.len() {
                return Err(malformed(
                    "axml string pool",
                    abs_offset,
                    "string offset extends past pool",
                ));
            }

            let s = if is_utf8 {
                decode_utf8_string(chunk_data, abs_offset)?
            } else {
                decode_utf16_string(chunk_data, abs_offset)?
            };
            strings.push(s);
        }

        Ok(StringPool { strings, is_utf8 })
    }

    pub fn get(&self, index: u32) -> Option<&str> {
        self.strings.get(index as usize).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(idx) = self.strings.iter().position(|existing| existing == s) {
            return idx as u32;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_owned());
        idx
    }

    pub fn set(&mut self, index: u32, value: String) {
        if let Some(entry) = self.strings.get_mut(index as usize) {
            *entry = value;
        }
    }
}

fn decode_utf8_string(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;

    if pos >= data.len() {
        return Ok(String::new());
    }

    let char_len = read_u8(data, pos, "axml utf8 length")?;
    pos += if char_len & 0x80 != 0 { 2 } else { 1 };

    if pos >= data.len() {
        return Ok(String::new());
    }

    let len_byte = read_u8(data, pos, "axml utf8 byte length")?;
    let byte_len = if len_byte & 0x80 != 0 {
        let next = read_u8(data, pos + 1, "axml utf8 byte length")? as usize;
        pos += 2;
        (((len_byte & 0x7F) as usize) << 8) | next
    } else {
        pos += 1;
        len_byte as usize
    };

    if pos + byte_len > data.len() {
        return Err(malformed(
            "axml utf8 string",
            pos,
            "string extends past end of pool",
        ));
    }

    let bytes = &data[pos..pos + byte_len];
    String::from_utf8(bytes.to_vec())
        .or_else(|_| reseam_dex::encoding::mutf8::decode_mutf8(bytes))
        .map_err(|_| invalid("axml utf8 string", "invalid UTF-8/MUTF-8 in string pool"))
}

fn decode_utf16_string(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;

    if pos + 2 > data.len() {
        return Ok(String::new());
    }

    let first = read_u16_le(data, pos, "axml utf16 length")?;
    let char_count = if first & 0x8000 != 0 {
        let next = read_u16_le(data, pos + 2, "axml utf16 length")? as usize;
        pos += 4;
        (((first & 0x7FFF) as usize) << 16) | next
    } else {
        pos += 2;
        first as usize
    };
    if char_count > MAX_UTF16_CODE_UNITS {
        return Err(invalid(
            "axml utf16 string",
            "string length exceeds safety limit",
        ));
    }

    if pos + char_count * 2 > data.len() {
        return Err(malformed(
            "axml utf16 string",
            pos,
            "string extends past end of pool",
        ));
    }

    let mut code_units = Vec::with_capacity(char_count);
    for i in 0..char_count {
        code_units.push(read_u16_le(data, pos + i * 2, "axml utf16 string")?);
    }

    String::from_utf16(&code_units)
        .map_err(|_| invalid("axml utf16 string", "invalid UTF-16 in string pool"))
}
