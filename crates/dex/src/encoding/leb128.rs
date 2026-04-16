// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{buffer_exhausted, invalid_leb128, Result};
use crate::types::header::ParseOptions;

pub fn read_uleb128(buf: &[u8], pos: usize) -> Result<(u32, usize)> {
    read_uleb128_with_opts(buf, pos, &ParseOptions::default())
}

pub fn read_uleb128_with_opts(buf: &[u8], pos: usize, opts: &ParseOptions) -> Result<(u32, usize)> {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    let mut i = 0;
    loop {
        if pos + i >= buf.len() {
            return Err(buffer_exhausted("leb128", pos + i));
        }
        let byte = buf[pos + i];
        i += 1;
        value |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if i >= 5 {
            return Err(invalid_leb128(pos));
        }
    }
    if !opts.lenient_leb128 && i != minimal_uleb128_len(value) {
        return Err(invalid_leb128(pos));
    }
    Ok((value, i))
}

pub fn read_sleb128(buf: &[u8], pos: usize) -> Result<(i32, usize)> {
    read_sleb128_with_opts(buf, pos, &ParseOptions::default())
}

pub fn read_sleb128_with_opts(buf: &[u8], pos: usize, opts: &ParseOptions) -> Result<(i32, usize)> {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    let mut i = 0;
    let mut byte;
    loop {
        if pos + i >= buf.len() {
            return Err(buffer_exhausted("leb128", pos + i));
        }
        byte = buf[pos + i];
        i += 1;
        value |= ((byte & 0x7F) as u32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
        if i >= 5 {
            return Err(invalid_leb128(pos));
        }
    }
    if shift < 32 && (byte & 0x40) != 0 {
        value |= !0u32 << shift;
    }
    let value = value as i32;
    if !opts.lenient_leb128 && i != minimal_sleb128_len(value) {
        return Err(invalid_leb128(pos));
    }
    Ok((value, i))
}

pub fn read_uleb128p1(buf: &[u8], pos: usize) -> Result<(Option<u32>, usize)> {
    read_uleb128p1_with_opts(buf, pos, &ParseOptions::default())
}

pub fn read_uleb128p1_with_opts(
    buf: &[u8],
    pos: usize,
    opts: &ParseOptions,
) -> Result<(Option<u32>, usize)> {
    let (raw, size) = read_uleb128_with_opts(buf, pos, opts)?;
    if raw == 0 {
        Ok((None, size))
    } else {
        Ok((Some(raw - 1), size))
    }
}

pub fn write_uleb128(buf: &mut Vec<u8>, mut value: u32) -> usize {
    let mut count = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        count += 1;
        if value == 0 {
            break;
        }
    }
    count
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
        } else {
            buf.push(byte | 0x80);
            count += 1;
        }
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
    let mut tmp = Vec::new();
    write_uleb128(&mut tmp, value)
}

fn minimal_sleb128_len(value: i32) -> usize {
    let mut tmp = Vec::new();
    write_sleb128(&mut tmp, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uleb128_roundtrip() {
        for &val in &[0u32, 1, 127, 128, 16383, 16384, 0x7FFFFFFF, u32::MAX] {
            let mut buf = Vec::new();
            write_uleb128(&mut buf, val);
            let (decoded, size) = read_uleb128(&buf, 0).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(size, buf.len());
        }
    }

    #[test]
    fn test_sleb128_roundtrip() {
        for &val in &[0i32, 1, -1, 63, -64, 8191, -8192, i32::MAX, i32::MIN] {
            let mut buf = Vec::new();
            write_sleb128(&mut buf, val);
            let (decoded, size) = read_sleb128(&buf, 0).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(size, buf.len());
        }
    }

    #[test]
    fn test_uleb128p1_roundtrip() {
        for val in [None, Some(0u32), Some(1), Some(u32::MAX - 1)] {
            let mut buf = Vec::new();
            write_uleb128p1(&mut buf, val);
            let (decoded, size) = read_uleb128p1(&buf, 0).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(size, buf.len());
        }
    }

    #[test]
    fn test_overlong_uleb128() {
        // Value 0 encoded as 2 bytes: 0x80 0x00
        let buf = [0x80, 0x00];
        let (val, size) = read_uleb128(&buf, 0).unwrap();
        assert_eq!(val, 0);
        assert_eq!(size, 2);
    }
}
