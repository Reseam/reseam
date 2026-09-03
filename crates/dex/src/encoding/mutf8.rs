// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{invalid_mutf8, Result};
use crate::types::header::ParseOptions;

pub fn decode_mutf8(bytes: &[u8]) -> Result<String> {
    decode_mutf8_with_opts(bytes, 0, &ParseOptions::default())
}

pub fn decode_mutf8_with_opts(bytes: &[u8], offset: usize, opts: &ParseOptions) -> Result<String> {
    let mut result = String::new();
    walk_mutf8(bytes, offset, opts, |c| result.push(c))?;
    Ok(result)
}

/// Decodes bytes that already passed [`utf16_units`] validation; malformed
/// sequences become U+FFFD instead of errors.
pub fn decode_mutf8_lossy(bytes: &[u8]) -> String {
    let opts = ParseOptions {
        lenient_mutf8: true,
        ..ParseOptions::default()
    };
    let mut result = String::new();
    walk_mutf8(bytes, 0, &opts, |c| result.push(c))
        .expect("lenient MUTF-8 decoding only fails on truncation, which validation rejects");
    result
}

/// Validates a MUTF-8 payload and returns its length in UTF-16 code units.
pub fn utf16_units(bytes: &[u8], offset: usize, opts: &ParseOptions) -> Result<u32> {
    let mut units = 0u32;
    walk_mutf8(bytes, offset, opts, |c| units += c.len_utf16() as u32)?;
    Ok(units)
}

fn walk_mutf8(
    bytes: &[u8],
    offset: usize,
    opts: &ParseOptions,
    mut emit: impl FnMut(char),
) -> Result<()> {
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 == 0 {
            break;
        }
        i += 1;
        if b0 & 0x80 == 0 {
            emit(b0 as char);
        } else if b0 & 0xE0 == 0xC0 {
            if i >= bytes.len() {
                return Err(invalid_mutf8(offset + i - 1, "truncated 2-byte sequence"));
            }
            let b1 = bytes[i];
            if b1 & 0xC0 != 0x80 {
                if opts.lenient_mutf8 {
                    emit('\u{FFFD}');
                    continue;
                }
                return Err(invalid_mutf8(offset + i, "invalid 2-byte continuation"));
            }
            i += 1;
            let cp = ((b0 as u32 & 0x1F) << 6) | (b1 as u32 & 0x3F);
            match char::from_u32(cp) {
                Some(c) => emit(c),
                None if opts.lenient_mutf8 => emit('\u{FFFD}'),
                None => return Err(invalid_mutf8(offset + i - 2, "invalid 2-byte code point")),
            }
        } else if b0 & 0xF0 == 0xE0 {
            if i + 1 >= bytes.len() {
                return Err(invalid_mutf8(offset + i - 1, "truncated 3-byte sequence"));
            }
            let b1 = bytes[i];
            let b2 = bytes[i + 1];
            if b1 & 0xC0 != 0x80 || b2 & 0xC0 != 0x80 {
                if opts.lenient_mutf8 {
                    emit('\u{FFFD}');
                    continue;
                }
                return Err(invalid_mutf8(offset + i, "invalid 3-byte continuation"));
            }
            i += 2;
            let cp = ((b0 as u32 & 0x0F) << 12) | ((b1 as u32 & 0x3F) << 6) | (b2 as u32 & 0x3F);

            if (0xD800..=0xDBFF).contains(&cp) {
                if i + 2 < bytes.len() && bytes[i] & 0xF0 == 0xE0 {
                    let b3 = bytes[i];
                    let b4 = bytes[i + 1];
                    let b5 = bytes[i + 2];
                    if b4 & 0xC0 != 0x80 || b5 & 0xC0 != 0x80 {
                        if opts.lenient_mutf8 {
                            emit('\u{FFFD}');
                            continue;
                        }
                        return Err(invalid_mutf8(offset + i, "invalid surrogate continuation"));
                    }
                    let cp2 =
                        ((b3 as u32 & 0x0F) << 12) | ((b4 as u32 & 0x3F) << 6) | (b5 as u32 & 0x3F);
                    if (0xDC00..=0xDFFF).contains(&cp2) {
                        i += 3;
                        let supplementary = 0x10000 + ((cp - 0xD800) << 10) + (cp2 - 0xDC00);
                        match char::from_u32(supplementary) {
                            Some(c) => emit(c),
                            None if opts.lenient_mutf8 => emit('\u{FFFD}'),
                            None => {
                                return Err(invalid_mutf8(
                                    offset + i - 3,
                                    "invalid supplementary code point",
                                ))
                            }
                        }
                        continue;
                    }
                }
                if opts.lenient_mutf8 {
                    emit('\u{FFFD}');
                } else {
                    return Err(invalid_mutf8(offset + i - 3, "unpaired high surrogate"));
                }
            } else if (0xDC00..=0xDFFF).contains(&cp) {
                if opts.lenient_mutf8 {
                    emit('\u{FFFD}');
                } else {
                    return Err(invalid_mutf8(offset + i - 3, "unpaired low surrogate"));
                }
            } else {
                match char::from_u32(cp) {
                    Some(c) => emit(c),
                    None if opts.lenient_mutf8 => emit('\u{FFFD}'),
                    None => return Err(invalid_mutf8(offset + i - 3, "invalid 3-byte code point")),
                }
            }
        } else if opts.lenient_mutf8 {
            emit('\u{FFFD}');
        } else {
            return Err(invalid_mutf8(offset + i - 1, "invalid start byte"));
        }
    }
    Ok(())
}

pub fn encode_mutf8(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        if cp == 0 {
            // Null character: encode as 0xC0 0x80
            buf.push(0xC0);
            buf.push(0x80);
        } else if cp >= 0x10000 {
            // Supplementary character: encode as surrogate pair, each in 3-byte MUTF-8
            let adjusted = cp - 0x10000;
            let high = 0xD800 + (adjusted >> 10);
            let low = 0xDC00 + (adjusted & 0x3FF);
            buf.push(0xE0 | ((high >> 12) as u8));
            buf.push(0x80 | (((high >> 6) & 0x3F) as u8));
            buf.push(0x80 | ((high & 0x3F) as u8));
            buf.push(0xE0 | ((low >> 12) as u8));
            buf.push(0x80 | (((low >> 6) & 0x3F) as u8));
            buf.push(0x80 | ((low & 0x3F) as u8));
        } else if cp >= 0x800 {
            buf.push(0xE0 | ((cp >> 12) as u8));
            buf.push(0x80 | (((cp >> 6) & 0x3F) as u8));
            buf.push(0x80 | ((cp & 0x3F) as u8));
        } else if cp >= 0x80 {
            buf.push(0xC0 | ((cp >> 6) as u8));
            buf.push(0x80 | ((cp & 0x3F) as u8));
        } else {
            buf.push(cp as u8);
        }
    }
    buf
}

/// Count the number of UTF-16 code units for a Rust string.
pub fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_roundtrip() {
        let s = "Hello, World!";
        let encoded = encode_mutf8(s);
        assert_eq!(encoded, s.as_bytes());
        let decoded = decode_mutf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_null_char() {
        let s = "\0";
        let encoded = encode_mutf8(s);
        assert_eq!(encoded, &[0xC0, 0x80]);
        let decoded = decode_mutf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_bmp_chars() {
        let s = "日本語";
        let encoded = encode_mutf8(s);
        let decoded = decode_mutf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_supplementary_char() {
        let s = "𝄞"; // U+1D11E MUSICAL SYMBOL G CLEF
        let encoded = encode_mutf8(s);
        assert_eq!(encoded.len(), 6); // surrogate pair, 3 bytes each
        let decoded = decode_mutf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_empty() {
        let encoded = encode_mutf8("");
        assert!(encoded.is_empty());
        let decoded = decode_mutf8(&encoded).unwrap();
        assert_eq!(decoded, "");
    }
}
