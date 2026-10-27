// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use reseam_apk::{AxmlDocument, StringEncoding};

#[test]
fn string_pools_find_a_value_in_either_encoding() -> TestResult {
    for encoding in [StringEncoding::Utf8, StringEncoding::Utf16] {
        let mut document = AxmlDocument::new(encoding);
        for value in ["X", "World"] {
            document.intern_string(value);
        }
        let parsed = AxmlDocument::parse(&document.serialize()?)?;
        for (value, expected) in [("World", true), ("X", true), ("Worl", false), ("Y", false)] {
            assert_eq!(
                parsed.string_pool().contains(value)?,
                expected,
                "{encoding:?} {value}"
            );
        }
    }
    Ok(())
}
