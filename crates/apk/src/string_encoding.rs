/// Android binary string pool encoding.
///
/// Used by both AXML string pools and resources.arsc string pools,
/// which share the same length-prefixed encoding format.

pub(crate) fn encode_utf8(out: &mut Vec<u8>, s: &str) {
    let char_len = s.chars().count();
    let byte_len = s.len();

    if char_len > 0x7F {
        out.push(((char_len >> 8) & 0x7F) as u8 | 0x80);
        out.push((char_len & 0xFF) as u8);
    } else {
        out.push(char_len as u8);
    }

    if byte_len > 0x7F {
        out.push(((byte_len >> 8) & 0x7F) as u8 | 0x80);
        out.push((byte_len & 0xFF) as u8);
    } else {
        out.push(byte_len as u8);
    }

    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

pub(crate) fn encode_utf16(out: &mut Vec<u8>, s: &str) {
    let code_units: Vec<u16> = s.encode_utf16().collect();
    let char_count = code_units.len();

    if char_count > 0x7FFF {
        out.extend_from_slice(&(((char_count >> 16) as u16) | 0x8000).to_le_bytes());
        out.extend_from_slice(&((char_count & 0xFFFF) as u16).to_le_bytes());
    } else {
        out.extend_from_slice(&(char_count as u16).to_le_bytes());
    }

    for cu in &code_units {
        out.extend_from_slice(&cu.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
}
