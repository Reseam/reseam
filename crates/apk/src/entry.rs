// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EntryName(Box<str>);

impl EntryName {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for EntryName {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}

impl From<String> for EntryName {
    fn from(name: String) -> Self {
        Self(name.into_boxed_str())
    }
}

impl std::borrow::Borrow<str> for EntryName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for EntryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

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

pub(crate) fn next_free_dex_name(used: &mut HashSet<EntryName>) -> EntryName {
    (1..=used.len() + 1)
        .map(|ordinal| {
            EntryName::from(if ordinal == 1 {
                "classes.dex".to_string()
            } else {
                format!("classes{ordinal}.dex")
            })
        })
        .find(|name| used.insert(name.clone()))
        .expect("N occupied names leave a free name among N+1 candidates")
}

pub(crate) fn is_native_library(name: &str) -> bool {
    let mut parts = name.split('/');
    parts.next() == Some("lib")
        && parts.next().is_some()
        && parts
            .next()
            .is_some_and(|file| file.as_bytes().ends_with(b".so"))
        && parts.next().is_none()
}

pub(crate) fn is_signature_entry(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let Some(file) = upper.strip_prefix("META-INF/") else {
        return false;
    };
    !file.contains('/')
        && (file == "MANIFEST.MF"
            || [".SF", ".RSA", ".DSA", ".EC"]
                .iter()
                .any(|suffix| file.ends_with(suffix)))
}
