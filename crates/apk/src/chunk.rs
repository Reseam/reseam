// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ResChunk_header` walking, shared by AXML and `resources.arsc`.

use std::ops::Range;

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::{malformed, Result};

pub(crate) const HEADER_LEN: usize = 8;

pub(crate) struct Chunk {
    pub kind: u16,
    pub header_size: usize,
    pub range: Range<usize>,
}

pub(crate) fn chunks(buf: &[u8], range: Range<usize>, section: &'static str) -> Result<Vec<Chunk>> {
    let mut out = Vec::new();
    let mut pos = range.start;
    while pos + HEADER_LEN <= range.end {
        let kind = read_u16_le(buf, pos, section)?;
        let header_size = read_u16_le(buf, pos + 2, section)? as usize;
        let size = read_u32_le(buf, pos + 4, section)? as usize;
        if size < HEADER_LEN
            || header_size < HEADER_LEN
            || header_size > size
            || pos + size > range.end
        {
            return Err(malformed(section, pos, "chunk extends past its container"));
        }
        out.push(Chunk {
            kind,
            header_size,
            range: pos..pos + size,
        });
        pos += size;
    }
    Ok(out)
}

pub(crate) fn chunk_end(buf: &[u8], offset: usize) -> Result<usize> {
    require_len(buf, offset, HEADER_LEN, "chunk header")?;
    let size = read_u32_le(buf, offset + 4, "chunk header")? as usize;
    if size < HEADER_LEN || offset + size > buf.len() {
        return Err(malformed(
            "chunk header",
            offset,
            "chunk extends past its container",
        ));
    }
    Ok(offset + size)
}

pub(crate) fn write_header(out: &mut Vec<u8>, kind: u16, header_size: u16, size: usize) {
    write_u16(out, kind);
    write_u16(out, header_size);
    write_u32(out, size as u32);
}
