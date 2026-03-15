use crate::error::{ApkError, Result};

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
        if chunk_data.len() < 20 {
            return Err(axml_err("String pool chunk too small"));
        }

        let string_count = read_u32(chunk_data, 0) as usize;
        let _style_count = read_u32(chunk_data, 4) as usize;
        let flags = read_u32(chunk_data, 8);
        let strings_start = read_u32(chunk_data, 12) as usize;
        let _styles_start = read_u32(chunk_data, 16) as usize;

        let is_utf8 = (flags & FLAG_UTF8) != 0;

        // String offset array starts at byte 20
        let offsets_start = 20;
        if chunk_data.len() < offsets_start + string_count * 4 {
            return Err(axml_err("String pool offset array truncated"));
        }

        let mut strings = Vec::with_capacity(string_count);

        // strings_start is relative to the chunk header start (8 bytes before chunk_data)
        // chunk_data starts at offset 8 within the chunk, so string data is at
        // strings_start - 8 relative to chunk_data
        let str_data_offset = strings_start.saturating_sub(8);

        for i in 0..string_count {
            let offset = read_u32(chunk_data, offsets_start + i * 4) as usize;
            let abs_offset = str_data_offset + offset;

            if abs_offset >= chunk_data.len() {
                strings.push(String::new());
                continue;
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

    /// Look up a string by index.
    pub fn get(&self, index: u32) -> Option<&str> {
        self.strings.get(index as usize).map(|s| s.as_str())
    }

    /// Number of strings in the pool.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// Decode a UTF-8 string from the string pool.
/// Format: charLen (1-2 bytes varint), byteLen (1-2 bytes varint), then UTF-8 bytes, then \0.
fn decode_utf8_string(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;

    if pos >= data.len() {
        return Ok(String::new());
    }

    // Skip char length (1 or 2 bytes)
    if data[pos] & 0x80 != 0 {
        pos += 2;
    } else {
        pos += 1;
    }

    if pos >= data.len() {
        return Ok(String::new());
    }

    // Read byte length (1 or 2 bytes)
    let byte_len;
    if data[pos] & 0x80 != 0 {
        if pos + 1 >= data.len() {
            return Ok(String::new());
        }
        byte_len = (((data[pos] & 0x7F) as usize) << 8) | (data[pos + 1] as usize);
        pos += 2;
    } else {
        byte_len = data[pos] as usize;
        pos += 1;
    }

    if pos + byte_len > data.len() {
        return Err(axml_err("UTF-8 string extends past end of pool"));
    }

    String::from_utf8(data[pos..pos + byte_len].to_vec())
        .map_err(|_| axml_err("Invalid UTF-8 in string pool"))
}

/// Decode a UTF-16LE string from the string pool.
/// Format: charLen (u16 or u32 if high bit set), then UTF-16LE code units, then \0\0.
fn decode_utf16_string(data: &[u8], offset: usize) -> Result<String> {
    let mut pos = offset;

    if pos + 2 > data.len() {
        return Ok(String::new());
    }

    // Read char count
    let char_count;
    let first = read_u16(data, pos);
    if first & 0x8000 != 0 {
        // High bit set: next u16 is the real count
        if pos + 4 > data.len() {
            return Ok(String::new());
        }
        char_count = (((first & 0x7FFF) as usize) << 16) | (read_u16(data, pos + 2) as usize);
        pos += 4;
    } else {
        char_count = first as usize;
        pos += 2;
    }

    if pos + char_count * 2 > data.len() {
        return Err(axml_err("UTF-16 string extends past end of pool"));
    }

    let code_units: Vec<u16> = (0..char_count)
        .map(|i| read_u16(data, pos + i * 2))
        .collect();

    String::from_utf16(&code_units).map_err(|_| axml_err("Invalid UTF-16 in string pool"))
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn axml_err(reason: &str) -> ApkError {
    ApkError::AxmlError {
        reason: reason.to_string(),
    }
}
