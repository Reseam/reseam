// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{COMPLEX_LEN, EntryValue, FLAG_COMPACT, FLAG_COMPLEX, ResEntry, SIMPLE_LEN};
use super::{MAP_ENTRY_LEN, MapEntry};
use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::error::{Result, malformed};
use crate::value::ResValue;

pub(in crate::resources) fn entry_len(chunk: &[u8], pos: usize) -> Result<usize> {
    require_len(chunk, pos, SIMPLE_LEN, "res entry")?;
    let flags = read_u16_le(chunk, pos + 2, "res entry")?;
    if flags & FLAG_COMPACT != 0 {
        if flags & FLAG_COMPLEX != 0 {
            return Err(malformed(
                "res entry",
                pos,
                "compact entry cannot be complex",
            ));
        }
        return Ok(SIMPLE_LEN);
    }
    let entry_size = read_u16_le(chunk, pos, "res entry")? as usize;
    let total = if flags & FLAG_COMPLEX != 0 {
        if entry_size < COMPLEX_LEN {
            return Err(malformed(
                "res entry",
                pos,
                "complex entry header is too short",
            ));
        }
        require_len(chunk, pos, entry_size, "complex res entry")?;
        let count = read_u32_le(chunk, pos + 12, "res entry count")? as usize;
        count
            .checked_mul(MAP_ENTRY_LEN)
            .and_then(|bytes| entry_size.checked_add(bytes))
            .ok_or_else(|| malformed("res entry", pos, "complex entry size overflows"))?
    } else {
        if entry_size < SIMPLE_LEN {
            return Err(malformed(
                "res entry",
                pos,
                "simple entry header is too short",
            ));
        }
        require_len(chunk, pos, entry_size + SIMPLE_LEN, "res entry")?;
        let value_size = read_u16_le(chunk, pos + entry_size, "res value")? as usize;
        if value_size < SIMPLE_LEN {
            return Err(malformed(
                "res value",
                pos + entry_size,
                "value is shorter than 8 bytes",
            ));
        }
        entry_size + value_size
    };
    require_len(chunk, pos, total, "res entry")?;
    Ok(total)
}

pub(in crate::resources) fn entry_head(bytes: &[u8]) -> Result<(u32, Option<ResValue>)> {
    let flags = read_u16_le(bytes, 2, "res entry")?;
    if flags & FLAG_COMPACT != 0 {
        let key = u32::from(read_u16_le(bytes, 0, "compact res entry")?);
        let value = ResValue::new(
            (flags >> 8) as u8,
            read_u32_le(bytes, 4, "compact res entry")?,
        );
        return Ok((key, Some(value)));
    }
    let key = read_u32_le(bytes, 4, "res entry")?;
    let value = if flags & FLAG_COMPLEX == 0 {
        let at = read_u16_le(bytes, 0, "res entry")? as usize;
        Some(ResValue::read(bytes, at, "res value")?)
    } else {
        None
    };
    Ok((key, value))
}

pub(in crate::resources) fn parse_entry(bytes: &[u8]) -> Result<ResEntry> {
    let flags = read_u16_le(bytes, 2, "res entry")?;
    let (key, simple) = entry_head(bytes)?;
    let value = if let Some(value) = simple {
        EntryValue::Simple(value)
    } else {
        let entry_size = read_u16_le(bytes, 0, "res entry")? as usize;
        let parent = read_u32_le(bytes, 8, "res entry parent")?;
        let count = read_u32_le(bytes, 12, "res entry count")? as usize;
        let entries = (0..count)
            .map(|i| {
                let at = entry_size + i * MAP_ENTRY_LEN;
                Ok(MapEntry {
                    name: read_u32_le(bytes, at, "map entry")?,
                    value: ResValue::read(bytes, at + 4, "map entry")?,
                })
            })
            .collect::<Result<_>>()?;
        EntryValue::Complex { parent, entries }
    };
    let flags = if flags & FLAG_COMPACT != 0 {
        flags & 0x00ff & !FLAG_COMPACT
    } else {
        flags
    };
    Ok(ResEntry { flags, key, value })
}
