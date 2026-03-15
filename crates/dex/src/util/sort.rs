use std::cmp::Ordering;

/// Compare two strings using DEX string sort order (UTF-16 code unit comparison).
pub fn dex_string_compare(a: &str, b: &str) -> Ordering {
    let mut a_units = a.encode_utf16();
    let mut b_units = b.encode_utf16();
    loop {
        match (a_units.next(), b_units.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(au), Some(bu)) => {
                let cmp = au.cmp(&bu);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_sort() {
        assert_eq!(dex_string_compare("abc", "abd"), Ordering::Less);
        assert_eq!(dex_string_compare("abc", "abc"), Ordering::Equal);
        assert_eq!(dex_string_compare("abd", "abc"), Ordering::Greater);
    }

    #[test]
    fn test_prefix_sort() {
        assert_eq!(dex_string_compare("ab", "abc"), Ordering::Less);
    }
}
