// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::str::FromStr;

use memchr::memmem::Finder;

use crate::error::ElfError;

/// Bytes to find in code, written as hex bytes separated by spaces, with `??` for any byte:
/// `"1f 20 ?? d5"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    bytes: Vec<Option<u8>>,
    /// The longest run of known bytes, which a search looks for first, and where it starts.
    anchor: Vec<u8>,
    anchor_start: usize,
}

impl Pattern {
    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    /// The offsets in `haystack` where the pattern matches, overlapping ones included.
    pub(crate) fn matches<'a>(&'a self, haystack: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
        let finder = Finder::new(&self.anchor);
        let mut from = 0;
        std::iter::from_fn(move || {
            while let Some(found) = haystack.get(from..).and_then(|rest| finder.find(rest)) {
                let at = from + found;
                from = at + 1;
                if let Some(start) = at.checked_sub(self.anchor_start)
                    && self.matches_at(haystack, start)
                {
                    return Some(start);
                }
            }
            None
        })
    }

    fn matches_at(&self, haystack: &[u8], start: usize) -> bool {
        haystack
            .get(start..start + self.len())
            .is_some_and(|bytes| {
                bytes
                    .iter()
                    .zip(&self.bytes)
                    .all(|(byte, expected)| expected.is_none_or(|expected| expected == *byte))
            })
    }
}

impl FromStr for Pattern {
    type Err = ElfError;

    fn from_str(text: &str) -> Result<Self, ElfError> {
        let invalid = |reason| ElfError::Pattern {
            pattern: text.to_owned(),
            reason,
        };
        let bytes = text
            .split_whitespace()
            .map(|token| match token {
                "??" => Ok(None),
                _ if token.len() == 2 && token.bytes().all(|digit| digit.is_ascii_hexdigit()) => {
                    Ok(Some(
                        u8::from_str_radix(token, 16).expect("two hex digits are a byte"),
                    ))
                }
                _ => Err(invalid("each byte is two hex digits or ??")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (anchor_start, anchor) = bytes
            .chunk_by(|a, b| a.is_some() == b.is_some())
            .scan(0, |start, run| {
                let run_start = *start;
                *start += run.len();
                Some((run_start, run))
            })
            .filter(|(_, run)| run[0].is_some())
            .max_by_key(|(_, run)| run.len())
            .map(|(start, run)| (start, run.iter().flatten().copied().collect()))
            .ok_or_else(|| invalid("a pattern needs at least one known byte"))?;
        Ok(Self {
            bytes,
            anchor,
            anchor_start,
        })
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes: Vec<String> = (self.bytes.iter())
            .map(|byte| byte.map_or_else(|| "??".to_owned(), |byte| format!("{byte:02x}")))
            .collect();
        f.write_str(&bytes.join(" "))
    }
}
