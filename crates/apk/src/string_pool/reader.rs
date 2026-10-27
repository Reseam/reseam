// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{FLAG_UTF8, HEADER_LEN, StringEncoding, StringPool, UTF8_LENGTH_MASK};
use crate::error::{Result, invalid, malformed};
use reseam_bytes::{read_u16, read_u32, require_len};
use reseam_storage::Bytes;
use std::borrow::Cow;
use std::ops::Range;

impl StringPool {
    pub(crate) fn parse(data: &Bytes, chunk: Range<usize>) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, HEADER_LEN, "string pool")?;
        let header_size = read_u16(buf, 2, "string pool")? as usize;
        if header_size < HEADER_LEN {
            return Err(malformed("string pool", 2, "header is too short"));
        }
        let string_count = read_u32(buf, 8, "string pool")? as usize;
        let style_count = read_u32(buf, 12, "string pool")? as usize;
        let flags = read_u32(buf, 16, "string pool")?;
        let strings_start = read_u32(buf, 20, "string pool")? as usize;
        let styles_start = read_u32(buf, 24, "string pool")? as usize;
        require_len(
            buf,
            header_size,
            string_count
                .checked_add(style_count)
                .and_then(|count| count.checked_mul(4))
                .ok_or_else(|| invalid("string pool", "offset table size overflows"))?,
            "string pool offsets",
        )?;
        let styles_in_range = styles_start >= strings_start && styles_start <= buf.len();
        if strings_start > buf.len() || (style_count > 0 && !styles_in_range) {
            return Err(malformed(
                "string pool",
                20,
                "string or style data start is outside pool",
            ));
        }

        let pool = Self {
            data: data.clone(),
            chunk: chunk.clone(),
            raw_len: string_count,
            style_count,
            encoding: if flags & FLAG_UTF8 != 0 {
                StringEncoding::Utf8
            } else {
                StringEncoding::Utf16
            },
            offsets_start: chunk.start + header_size,
            strings_start: chunk.start + strings_start,
            styles_start: chunk.start
                + if style_count > 0 {
                    styles_start
                } else {
                    buf.len()
                },
            ..Self::default()
        };
        Ok(pool)
    }
    pub(super) fn raw(&self, i: usize) -> Result<Cow<'_, str>> {
        let (pool, abs) = self.raw_at(i)?;
        if self.encoding == StringEncoding::Utf8 {
            decode_utf8(pool, abs)
        } else {
            decode_utf16(pool, abs).map(Cow::Owned)
        }
    }

    pub(super) fn raw_equals(&self, i: usize, value: &str) -> Result<bool> {
        let (pool, abs) = self.raw_at(i)?;
        if self.encoding == StringEncoding::Utf8 {
            return Ok(decode_utf8(pool, abs)? == value);
        }
        let (start, char_count) = utf16_span(pool, abs)?;
        if char_count != value.encode_utf16().count() {
            return Ok(false);
        }
        Ok(pool[start..start + char_count * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .eq(value.encode_utf16()))
    }

    fn raw_at(&self, i: usize) -> Result<(&[u8], usize)> {
        let data = self.data.as_bytes();
        let offset = read_u32(data, self.offsets_start + i * 4, "string offset")? as usize;
        let abs = self
            .strings_start
            .checked_add(offset)
            .ok_or_else(|| invalid("string pool", "string offset overflows"))?;
        if abs >= self.styles_start {
            return Err(malformed(
                "string pool",
                abs,
                "string offset extends past pool",
            ));
        }
        Ok((&data[..self.styles_start], abs))
    }
}

fn decode_utf8(data: &[u8], offset: usize) -> Result<Cow<'_, str>> {
    let mut pos = offset;
    let units = decode_len_u8(data, &mut pos)?;
    let encoded_len = decode_len_u8(data, &mut pos)?;
    require_len(data, pos, encoded_len + 1, "utf8 string")?;
    let tail = &data[pos..];
    // AAPT truncates both lengths to 15 bits; Android probes the terminator
    // at lengths congruent to the encoded byte count.
    let byte_len = (encoded_len..tail.len())
        .step_by(UTF8_LENGTH_MASK + 1)
        .find(|&end| tail[end] == 0)
        .ok_or_else(|| malformed("utf8 string", pos, "string is not terminated"))?;
    let bytes = &tail[..byte_len];
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(
            reseam_dex::decode_mutf8(bytes)
                .map_err(|_| invalid("utf8 string", "invalid UTF-8/MUTF-8"))?,
        ),
    };
    let actual_units = if text.is_ascii() {
        text.len()
    } else {
        text.encode_utf16().count()
    };
    if actual_units & UTF8_LENGTH_MASK != units {
        return Err(malformed(
            "utf8 string",
            offset,
            "decoded UTF-16 length does not match",
        ));
    }
    Ok(text)
}

fn decode_len_u8(data: &[u8], pos: &mut usize) -> Result<usize> {
    require_len(data, *pos, 1, "utf8 length")?;
    let first = data[*pos];
    let length = if first & 0x80 == 0 {
        usize::from(first)
    } else {
        require_len(data, *pos, 2, "utf8 length")?;
        *pos += 1;
        (usize::from(first & 0x7f) << 8) | usize::from(data[*pos])
    };
    *pos += 1;
    Ok(length)
}

fn utf16_span(data: &[u8], offset: usize) -> Result<(usize, usize)> {
    let mut pos = offset;
    let first = read_u16(data, pos, "utf16 length")?;
    let char_count = if first & 0x8000 != 0 {
        let next = read_u16(data, pos + 2, "utf16 length")? as usize;
        pos += 4;
        (((first & 0x7FFF) as usize) << 16) | next
    } else {
        pos += 2;
        first as usize
    };
    if char_count > (data.len() - pos) / 2 {
        return Err(malformed("utf16 string", pos, "string extends past pool"));
    }
    Ok((pos, char_count))
}

fn decode_utf16(data: &[u8], offset: usize) -> Result<String> {
    let (pos, char_count) = utf16_span(data, offset)?;
    let units = data[pos..pos + char_count * 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit));
    Ok(char::decode_utf16(units)
        .map(|character| character.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect())
}
