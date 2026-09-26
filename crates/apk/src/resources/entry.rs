// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::error::{malformed, Result};
use crate::value::ResValue;

const FLAG_COMPLEX: u16 = 0x0001;
pub(super) const FLAG_COMPACT: u16 = 0x0008;
const SIMPLE_LEN: usize = 8;
const COMPLEX_LEN: usize = 16;
const MAP_ENTRY_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct ResEntry {
    pub flags: u16,
    pub key: u32,
    pub value: EntryValue,
}

#[derive(Debug, Clone)]
pub enum EntryValue {
    Simple(ResValue),
    Complex { parent: u32, entries: Vec<MapEntry> },
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub name: u32,
    pub value: ResValue,
}

pub(super) fn entry_len(chunk: &[u8], pos: usize) -> Result<usize> {
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

/// The key and, for a simple entry, the value, without decoding a map.
pub(super) fn entry_head(bytes: &[u8]) -> Option<(u32, Option<ResValue>)> {
    let flags = read_u16_le(bytes, 2, "res entry").ok()?;
    if flags & FLAG_COMPACT != 0 {
        let key = read_u16_le(bytes, 0, "compact res entry").ok()? as u32;
        let value = ResValue::new(
            (flags >> 8) as u8,
            read_u32_le(bytes, 4, "compact res entry").ok()?,
        );
        return Some((key, Some(value)));
    }
    let key = read_u32_le(bytes, 4, "res entry").ok()?;
    let value = if flags & FLAG_COMPLEX == 0 {
        let at = read_u16_le(bytes, 0, "res entry").ok()? as usize;
        Some(ResValue::new(
            *bytes.get(at + 3)?,
            read_u32_le(bytes, at + 4, "res value").ok()?,
        ))
    } else {
        None
    };
    Some((key, value))
}

pub(super) fn parse_entry(bytes: &[u8]) -> Result<ResEntry> {
    let flags = read_u16_le(bytes, 2, "res entry")?;
    if flags & FLAG_COMPACT != 0 {
        return Ok(ResEntry {
            flags: flags & 0x00ff & !FLAG_COMPACT,
            key: read_u16_le(bytes, 0, "compact res entry")? as u32,
            value: EntryValue::Simple(ResValue::new(
                (flags >> 8) as u8,
                read_u32_le(bytes, 4, "compact res entry")?,
            )),
        });
    }
    let key = read_u32_le(bytes, 4, "res entry")?;
    let entry_size = read_u16_le(bytes, 0, "res entry")? as usize;
    let value = if flags & FLAG_COMPLEX != 0 {
        let parent = read_u32_le(bytes, 8, "res entry parent")?;
        let count = read_u32_le(bytes, 12, "res entry count")? as usize;
        let entries = (0..count)
            .map(|i| {
                let at = entry_size + i * MAP_ENTRY_LEN;
                Ok(MapEntry {
                    name: read_u32_le(bytes, at, "map entry")?,
                    value: ResValue::new(bytes[at + 7], read_u32_le(bytes, at + 8, "map entry")?),
                })
            })
            .collect::<Result<_>>()?;
        EntryValue::Complex { parent, entries }
    } else {
        EntryValue::Simple(ResValue::new(
            bytes[entry_size + 3],
            read_u32_le(bytes, entry_size + 4, "res value")?,
        ))
    };
    Ok(ResEntry { flags, key, value })
}

pub(super) fn serialize_entry(out: &mut Vec<u8>, entry: &ResEntry) {
    match &entry.value {
        EntryValue::Simple(value) => {
            write_u16(out, SIMPLE_LEN as u16);
            write_u16(out, entry.flags & !(FLAG_COMPLEX | FLAG_COMPACT));
            write_u32(out, entry.key);
            write_value(out, *value);
        }
        EntryValue::Complex { parent, entries } => {
            write_u16(out, COMPLEX_LEN as u16);
            write_u16(out, (entry.flags | FLAG_COMPLEX) & !FLAG_COMPACT);
            write_u32(out, entry.key);
            write_u32(out, *parent);
            write_u32(out, entries.len() as u32);
            for map_entry in entries {
                write_u32(out, map_entry.name);
                write_value(out, map_entry.value);
            }
        }
    }
}

fn write_value(out: &mut Vec<u8>, value: ResValue) {
    write_u16(out, 8);
    out.push(0);
    out.push(value.kind);
    write_u32(out, value.data);
}
