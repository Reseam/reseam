// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Entry names inside an APK and what they mean to the platform.

use std::collections::HashSet;

pub const MANIFEST_ENTRY: &str = "AndroidManifest.xml";
pub const RESOURCES_ENTRY: &str = "resources.arsc";

/// `classes.dex` is ordinal 1, `classesN.dex` is ordinal N.
pub fn dex_ordinal(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("classes")?.strip_suffix(".dex")?;
    match rest {
        "" => Some(1),
        digits if digits.bytes().all(|b| b.is_ascii_digit()) => {
            digits.parse().ok().filter(|&n| n >= 2)
        }
        _ => None,
    }
}

pub(crate) fn dex_entry_name(ordinal: u32) -> String {
    match ordinal {
        1 => "classes.dex".into(),
        n => format!("classes{n}.dex"),
    }
}

pub(crate) fn next_free_dex_name(used: &mut HashSet<String>) -> String {
    (1..)
        .map(dex_entry_name)
        .find(|name| used.insert(name.clone()))
        .expect("unbounded ordinals")
}

pub(crate) fn is_native_library(name: &str) -> bool {
    let mut parts = name.split('/');
    parts.next() == Some("lib")
        && parts.next().is_some()
        && parts.next().is_some_and(|file| file.ends_with(".so"))
        && parts.next().is_none()
}

pub(crate) fn is_signature_entry(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper == "META-INF/MANIFEST.MF"
        || [".SF", ".RSA", ".DSA", ".EC"]
            .iter()
            .any(|suffix| upper.ends_with(suffix))
}
