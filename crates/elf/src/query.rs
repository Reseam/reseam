// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;

use crate::Pattern;

/// The constraints a function must meet, combined with AND. Without `nth` exactly one function
/// may match; with it, the match at that index in start-address order is taken. A query without
/// a symbol, string or pattern matches nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<'a> {
    /// The function is defined under this symbol.
    pub symbol: Option<&'a str>,
    /// The function's code computes the address of each string in loaded data.
    pub strings: &'a [&'a str],
    /// The function's code holds each pattern.
    pub patterns: &'a [Pattern],
    pub nth: Option<usize>,
}

impl fmt::Display for Query<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(symbol) = self.symbol {
            parts.push(format!("symbol {symbol}"));
        }
        if !self.strings.is_empty() {
            parts.push(format!("strings {:?}", self.strings));
        }
        for pattern in self.patterns {
            parts.push(format!("bytes {pattern}"));
        }
        if let Some(nth) = self.nth {
            parts.push(format!("match {nth}"));
        }
        if parts.is_empty() {
            f.write_str("no constraint")
        } else {
            f.write_str(&parts.join(", "))
        }
    }
}
