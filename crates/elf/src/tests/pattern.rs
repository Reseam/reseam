// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use pretty_assertions::assert_eq;

use crate::{ElfError, Pattern};

#[test]
fn parses_byte_patterns() {
    let valid = [("0a 2A ?? 34", "0a 2a ?? 34"), ("  ff\t?? 00 ", "ff ?? 00")];
    for (text, shown) in valid {
        let pattern: Pattern = text.parse().expect("valid pattern");
        assert_eq!(pattern.to_string(), shown);
    }
    for text in ["", "?? ??", "0a2a", "+f", "zz", "0a ?"] {
        assert!(
            matches!(text.parse::<Pattern>(), Err(ElfError::Pattern { .. })),
            "{text:?}"
        );
    }
}

/// The search looks for the longest run of known bytes first. Its first occurrence here starts no
/// match, and the one that does overlaps it.
#[test]
fn finds_matches_whose_known_bytes_overlap_a_false_start() {
    let pattern: Pattern = "00 00 ?? 01".parse().expect("valid pattern");
    let matches: Vec<usize> = pattern.matches(&[0, 0, 0, 5, 1]).collect();
    assert_eq!(matches, [1]);
}
