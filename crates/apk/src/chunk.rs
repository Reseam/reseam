// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::{Result, malformed};

pub(crate) const HEADER_LEN: usize = 8;

pub(crate) struct Chunk {
    pub kind: u16,
    pub header_size: usize,
    pub range: Range<usize>,
}

pub(crate) fn chunks(buf: &[u8], range: Range<usize>, section: &'static str) -> Result<Vec<Chunk>> {
    if range.start > range.end || range.end > buf.len() {
        return Err(malformed(
            section,
            range.start,
            "chunk container extends past input",
        ));
    }
    let mut out = Vec::new();
    let mut pos = range.start;
    while range.end - pos >= HEADER_LEN {
        let kind = read_u16_le(buf, pos, section)?;
        let header_size = read_u16_le(buf, pos + 2, section)? as usize;
        let size = read_u32_le(buf, pos + 4, section)? as usize;
        let Some(end) = pos.checked_add(size).filter(|&end| {
            end <= range.end
                && size >= HEADER_LEN
                && header_size >= HEADER_LEN
                && header_size <= size
        }) else {
            return Err(malformed(section, pos, "chunk extends past its container"));
        };
        out.push(Chunk {
            kind,
            header_size,
            range: pos..end,
        });
        pos = end;
    }
    Ok(out)
}

pub(crate) fn chunk_end(buf: &[u8], offset: usize) -> Result<usize> {
    require_len(buf, offset, HEADER_LEN, "chunk header")?;
    let size = read_u32_le(buf, offset + 4, "chunk header")? as usize;
    let Some(end) = offset
        .checked_add(size)
        .filter(|&end| end <= buf.len() && size >= HEADER_LEN)
    else {
        return Err(malformed(
            "chunk header",
            offset,
            "chunk extends past its container",
        ));
    };
    Ok(end)
}

pub(crate) fn write_header(out: &mut Vec<u8>, kind: u16, header_size: u16, size: usize) {
    write_u16(out, kind);
    write_u16(out, header_size);
    write_u32(out, size as u32);
}
