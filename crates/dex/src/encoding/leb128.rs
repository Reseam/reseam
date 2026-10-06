// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, buffer_exhausted, invalid_leb128};
use crate::types::header::ParseOptions;

pub fn read_uleb128(buf: &[u8], pos: usize) -> Result<(u32, usize)> {
    read_uleb128_with_opts(buf, pos, ParseOptions::default())
}

pub fn read_uleb128_with_opts(buf: &[u8], pos: usize, opts: ParseOptions) -> Result<(u32, usize)> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let at = pos
            .checked_add(i)
            .ok_or_else(|| buffer_exhausted("leb128", pos))?;
        let byte = *buf.get(at).ok_or_else(|| buffer_exhausted("leb128", at))?;
        value |= u32::from(byte & 0x7F) << (i * 7);
        if byte & 0x80 == 0 {
            if !(opts.leb128 == crate::types::header::Validation::Permissive)
                && i + 1 != minimal_uleb128_len(value)
            {
                return Err(invalid_leb128(pos));
            }
            return Ok((value, i + 1));
        }
    }
    Err(invalid_leb128(pos))
}

pub fn read_sleb128(buf: &[u8], pos: usize) -> Result<(i32, usize)> {
    read_sleb128_with_opts(buf, pos, ParseOptions::default())
}

pub fn read_sleb128_with_opts(buf: &[u8], pos: usize, opts: ParseOptions) -> Result<(i32, usize)> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let at = pos
            .checked_add(i)
            .ok_or_else(|| buffer_exhausted("leb128", pos))?;
        let byte = *buf.get(at).ok_or_else(|| buffer_exhausted("leb128", at))?;
        let shift = (i + 1) * 7;
        value |= u32::from(byte & 0x7F) << (i * 7);
        if byte & 0x80 == 0 {
            if shift < 32 && (byte & 0x40) != 0 {
                value |= !0u32 << shift;
            }
            let value = value as i32;
            if !(opts.leb128 == crate::types::header::Validation::Permissive)
                && i + 1 != minimal_sleb128_len(value)
            {
                return Err(invalid_leb128(pos));
            }
            return Ok((value, i + 1));
        }
    }
    Err(invalid_leb128(pos))
}

pub fn read_uleb128p1(buf: &[u8], pos: usize) -> Result<(Option<u32>, usize)> {
    read_uleb128p1_with_opts(buf, pos, ParseOptions::default())
}

pub fn read_uleb128p1_with_opts(
    buf: &[u8],
    pos: usize,
    opts: ParseOptions,
) -> Result<(Option<u32>, usize)> {
    let (raw, size) = read_uleb128_with_opts(buf, pos, opts)?;
    if raw == 0 {
        Ok((None, size))
    } else {
        Ok((Some(raw - 1), size))
    }
}

pub fn write_uleb128(buf: &mut Vec<u8>, value: u32) -> usize {
    let (bytes, len) = encode_uleb128(value);
    buf.extend_from_slice(&bytes[..len]);
    len
}

pub fn encode_uleb128(mut value: u32) -> ([u8; 5], usize) {
    let mut bytes = [0u8; 5];
    let mut len = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes[len] = byte;
        len += 1;
        if value == 0 {
            break;
        }
    }
    (bytes, len)
}

pub fn write_sleb128(buf: &mut Vec<u8>, mut value: i32) -> usize {
    let mut count = 0;
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            buf.push(byte);
            count += 1;
            break;
        }
        buf.push(byte | 0x80);
        count += 1;
    }
    count
}

pub fn write_uleb128p1(buf: &mut Vec<u8>, value: Option<u32>) -> usize {
    match value {
        None => write_uleb128(buf, 0),
        Some(v) => write_uleb128(buf, v + 1),
    }
}

fn minimal_uleb128_len(value: u32) -> usize {
    (32 - value.leading_zeros()).max(1).div_ceil(7) as usize
}

fn minimal_sleb128_len(value: i32) -> usize {
    let significant_bits = 33 - (value ^ (value >> 31)).leading_zeros();
    significant_bits.div_ceil(7) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leb128_encodings_preserve_values_and_accept_android_padding() {
        for signed in [
            0,
            1,
            -1,
            63,
            -64,
            127,
            128,
            8191,
            -8192,
            16383,
            16384,
            i32::MAX,
            i32::MIN,
        ] {
            let value = signed as u32;
            let mut bytes = Vec::new();
            write_uleb128(&mut bytes, value);
            assert_eq!(read_uleb128(&bytes, 0).unwrap(), (value, bytes.len()));
            bytes.clear();
            write_sleb128(&mut bytes, signed);
            assert_eq!(read_sleb128(&bytes, 0).unwrap(), (signed, bytes.len()));
        }
        for value in [None, Some(0), Some(1), Some(u32::MAX - 1)] {
            let mut bytes = Vec::new();
            write_uleb128p1(&mut bytes, value);
            assert_eq!(read_uleb128p1(&bytes, 0).unwrap(), (value, bytes.len()));
        }
        for (bytes, value) in [
            (&[0x80, 0][..], 0),
            (&[0x80, 0x80, 0][..], 0),
            (&[0x81, 0][..], 1),
            (&[0x81, 0x80, 0][..], 1),
        ] {
            assert_eq!(read_uleb128(bytes, 0).unwrap(), (value, bytes.len()));
        }
    }
}
