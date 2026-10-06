// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[derive(Clone, Copy)]
enum TypePosition {
    Parameter,
    Return,
}

fn type_descriptor_len(desc: &str, position: TypePosition) -> Option<usize> {
    let bytes = desc.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut i = 0;
    while i < bytes.len() && bytes[i] == b'[' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    match bytes[i] {
        b'V' => {
            if matches!(position, TypePosition::Parameter) || i != 0 {
                return None;
            }
            Some(1)
        }
        b'Z' | b'B' | b'S' | b'C' | b'I' | b'J' | b'F' | b'D' => Some(i + 1),
        b'L' => {
            let semi = desc[i..].find(';')?;
            let len = i + semi + 1;
            if semi == 1 {
                return None;
            }
            Some(len)
        }
        _ => None,
    }
}

pub fn is_type_descriptor(desc: &str) -> bool {
    matches!(type_descriptor_len(desc, TypePosition::Return), Some(len) if len == desc.len())
}

/// Parse a method descriptor like "(II)V" into (`param_types`, `return_type`).
pub fn parse_method_descriptor(desc: &str) -> Option<(Vec<&str>, &str)> {
    if !desc.starts_with('(') {
        return None;
    }
    let close = desc.find(')')?;
    let params_str = &desc[1..close];
    let return_type = &desc[close + 1..];

    let mut params = Vec::new();
    let mut i = 0;
    while i < params_str.len() {
        let len = type_descriptor_len(&params_str[i..], TypePosition::Parameter)?;
        if len == 0 {
            return None;
        }
        params.push(&params_str[i..i + len]);
        i += len;
    }

    let return_len = type_descriptor_len(return_type, TypePosition::Return)?;
    if return_len != return_type.len() {
        return None;
    }

    Some((params, return_type))
}

/// Generate a shorty descriptor from a method descriptor.
pub fn shorty_from_descriptor(desc: &str) -> Option<String> {
    let (params, ret) = parse_method_descriptor(desc)?;
    Some(shorty_from_parts(&params, ret))
}

pub(crate) fn shorty_from_parts(params: &[&str], ret: &str) -> String {
    let mut shorty = String::with_capacity(1 + params.len());
    shorty.push(shorty_char(ret));
    shorty.extend(params.iter().map(|param| shorty_char(param)));
    shorty
}

fn shorty_char(type_desc: &str) -> char {
    match type_desc.as_bytes()[0] {
        b'L' | b'[' => 'L',
        primitive => primitive as char,
    }
}
