// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid_mutf8};
use crate::types::header::ParseOptions;

pub fn decode_mutf8(bytes: &[u8]) -> Result<String> {
    decode_mutf8_with_opts(bytes, 0, ParseOptions::default())
}

/// Decodes DEX UTF-16 units for display. Unpaired surrogates display as U+FFFD;
/// their exact identity remains available through the encoded payload.
pub fn decode_mutf8_with_opts(bytes: &[u8], offset: usize, opts: ParseOptions) -> Result<String> {
    let mut error = None;
    let units = checked_units(bytes, offset, opts)
        .map_while(|unit| match unit {
            Ok(unit) => Some(unit),
            Err(cause) => {
                error = Some(cause);
                None
            }
        })
        .fuse();
    let text = char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    match error {
        Some(error) => Err(error),
        None => Ok(text),
    }
}

/// Decodes a payload for display, replacing malformed sequences and unpaired surrogates.
pub fn decode_mutf8_lossy(bytes: &[u8]) -> String {
    char::decode_utf16(mutf8_units(bytes))
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Validates encoding and counts UTF-16 units, including unpaired surrogates accepted by ART.
pub fn utf16_units(bytes: &[u8], offset: usize, opts: ParseOptions) -> Result<u32> {
    checked_units(bytes, offset, opts).try_fold(0u32, |count, unit| {
        unit?;
        count
            .checked_add(1)
            .ok_or_else(|| invalid_mutf8(offset, "UTF-16 length exceeds u32"))
    })
}

/// Iterates exact UTF-16 units; malformed byte groups yield the replacement unit.
pub fn mutf8_units(bytes: &[u8]) -> impl Iterator<Item = u16> + '_ {
    checked_units(bytes, 0, ParseOptions::default()).map(|unit| unit.unwrap_or(0xfffd))
}

fn checked_units(
    bytes: &[u8],
    offset: usize,
    opts: ParseOptions,
) -> impl Iterator<Item = Result<u16>> + use<'_> {
    let lenient = opts.mutf8 == crate::types::header::Validation::Permissive;
    let mut position = 0;
    std::iter::from_fn(move || {
        let lead = *bytes.get(position)?;
        if lead == 0 {
            return None;
        }
        let start = position;
        position += 1;
        let (width, mask) = match lead {
            0x01..=0x7f => return Some(Ok(u16::from(lead))),
            0xc0..=0xdf => (2, 0x1f),
            0xe0..=0xef => (3, 0x0f),
            _ => {
                return Some(if lenient {
                    Ok(0xfffd)
                } else {
                    Err(invalid_mutf8(offset + start, "invalid start byte"))
                });
            }
        };
        let Some(group) = bytes.get(start..start + width) else {
            position = bytes.len();
            return Some(Err(invalid_mutf8(offset + start, "truncated sequence")));
        };
        if !group[1..].iter().all(|b| b & 0xc0 == 0x80) {
            return Some(if lenient {
                Ok(0xfffd)
            } else {
                Err(invalid_mutf8(offset + start, "invalid continuation"))
            });
        }
        position = start + width;
        Some(Ok(group[1..]
            .iter()
            .fold(u16::from(lead & mask), |unit, b| {
                (unit << 6) | u16::from(b & 0x3f)
            })))
    })
}

pub fn encode_mutf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        match unit {
            1..=0x7f => out.push(unit as u8),
            0..=0x7ff => out.extend([0xc0 | (unit >> 6) as u8, 0x80 | (unit & 0x3f) as u8]),
            _ => out.extend([
                0xe0 | (unit >> 12) as u8,
                0x80 | ((unit >> 6) & 0x3f) as u8,
                0x80 | (unit & 0x3f) as u8,
            ]),
        }
    }
    out
}

pub fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}
