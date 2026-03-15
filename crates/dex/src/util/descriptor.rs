fn type_descriptor_len(desc: &str, allow_void: bool) -> Option<usize> {
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
            if !allow_void || i != 0 {
                return None;
            }
            Some(1)
        }
        b'Z' | b'B' | b'S' | b'C' | b'I' | b'J' | b'F' | b'D' => Some(i + 1),
        b'L' => {
            let semi = desc[i..].find(';')?;
            let len = i + semi + 1;
            if len == i + 1 {
                return None;
            }
            Some(len)
        }
        _ => None,
    }
}

pub fn is_type_descriptor(desc: &str) -> bool {
    matches!(type_descriptor_len(desc, true), Some(len) if len == desc.len())
}

/// Parse a method descriptor like "(II)V" into (param_types, return_type).
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
        let len = type_descriptor_len(&params_str[i..], false)?;
        if len == 0 {
            return None;
        }
        params.push(&params_str[i..i + len]);
        i += len;
    }

    let return_len = type_descriptor_len(return_type, true)?;
    if return_len != return_type.len() {
        return None;
    }

    Some((params, return_type))
}

/// Generate a shorty descriptor from a method descriptor.
pub fn shorty_from_descriptor(desc: &str) -> Option<String> {
    let (params, ret) = parse_method_descriptor(desc)?;
    let mut shorty = String::with_capacity(1 + params.len());
    shorty.push(shorty_char(ret)?);
    for p in &params {
        shorty.push(shorty_char(p)?);
    }
    Some(shorty)
}

fn shorty_char(type_desc: &str) -> Option<char> {
    let first = type_desc.as_bytes().first()?;
    match first {
        b'V' => Some('V'),
        b'Z' => Some('Z'),
        b'B' => Some('B'),
        b'S' => Some('S'),
        b'C' => Some('C'),
        b'I' => Some('I'),
        b'J' => Some('J'),
        b'F' => Some('F'),
        b'D' => Some('D'),
        b'L' | b'[' => Some('L'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_descriptor() {
        let (params, ret) = parse_method_descriptor("(II)V").unwrap();
        assert_eq!(params, vec!["I", "I"]);
        assert_eq!(ret, "V");
    }

    #[test]
    fn test_parse_complex() {
        let (params, ret) = parse_method_descriptor("(Ljava/lang/String;[BI)Z").unwrap();
        assert_eq!(params, vec!["Ljava/lang/String;", "[B", "I"]);
        assert_eq!(ret, "Z");
    }

    #[test]
    fn test_shorty() {
        assert_eq!(shorty_from_descriptor("(II)V").unwrap(), "VII");
        assert_eq!(
            shorty_from_descriptor("(Ljava/lang/String;[BD)I").unwrap(),
            "ILLD"
        );
    }

    #[test]
    fn test_invalid_type_descriptors() {
        assert!(!is_type_descriptor(""));
        assert!(!is_type_descriptor("Lfoo"));
        assert!(!is_type_descriptor("VV"));
        assert!(!is_type_descriptor("[V"));
    }

    #[test]
    fn test_invalid_method_descriptors() {
        assert!(parse_method_descriptor("(V)V").is_none());
        assert!(parse_method_descriptor("(I)VV").is_none());
        assert!(parse_method_descriptor("(Ljava/lang/String)V").is_none());
    }
}
