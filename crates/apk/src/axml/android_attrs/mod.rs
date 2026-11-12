// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::LazyLock;

use crate::resources::{AttrFormats, AttrSymbol, ResourceName};

mod generated;

use generated::{ANDROID_ATTR_SYMBOLS, ANDROID_ATTRS, ANDROID_RESOURCES};
pub use generated::{
    ATTR_CONFIG_CHANGES, ATTR_DRAWABLE, ATTR_ENABLED, ATTR_ICON, ATTR_LABEL, ATTR_MIME_TYPE,
    ATTR_MIN_SDK_VERSION, ATTR_MULTI_ARCH, ATTR_NAME, ATTR_SPLIT_NAME, ATTR_SPLIT_TYPES,
    ATTR_TARGET_ACTIVITY, ATTR_VERSION_CODE, ATTR_VERSION_NAME,
};

/// The resource id of an `android:` attribute in Android 36's public attribute table.
pub fn android_attr_res_id(name: &str) -> Option<u32> {
    ANDROID_ATTRS
        .binary_search_by_key(&name, |(attr, _)| *attr)
        .ok()
        .map(|index| ANDROID_ATTRS[index].1.id)
}

/// The resource id of a public framework resource in Android 36, such as `color/white`.
pub fn android_res_id(type_name: &str, name: &str) -> Option<u32> {
    if type_name == "attr" {
        return android_attr_res_id(name);
    }
    ANDROID_RESOURCES
        .binary_search_by_key(&(type_name, name), |(name, _)| *name)
        .ok()
        .map(|index| ANDROID_RESOURCES[index].1)
}

/// The name of public framework resource `id`, attributes included.
pub fn android_res_name(id: u32) -> Option<ResourceName> {
    let index = NAMES_BY_ID.binary_search_by_key(&id, |&(id, ..)| id).ok()?;
    let (_, type_name, entry) = NAMES_BY_ID[index];
    Some(ResourceName {
        package: Some("android".to_owned()),
        type_name: type_name.to_owned(),
        entry: entry.to_owned(),
    })
}

/// The names public framework attribute `attr_id` gives `value`: its enum
/// name, or the flag names whose bits make it up exactly, joined with `|`,
/// widest flags first as aapt reads them. Names that alias one value set the
/// same thing; the first in alphabetical order is used.
pub fn android_attr_value_name(attr_id: u32, value: u32) -> Option<String> {
    let formats = android_attr_formats(attr_id)?;
    let start = ANDROID_ATTR_SYMBOLS.partition_point(|((id, _), _)| *id < attr_id);
    let symbols: Vec<(&str, u32)> = ANDROID_ATTR_SYMBOLS[start..]
        .iter()
        .take_while(|((id, _), _)| *id == attr_id)
        .map(|((_, name), value)| (*name, *value))
        .collect();
    let exact = symbols.iter().find(|&&(_, symbol)| symbol == value);
    if !formats.contains(AttrFormats::FLAGS) || exact.is_some() {
        return exact.map(|(name, _)| (*name).to_owned());
    }
    let mut flags: Vec<&(&str, u32)> = symbols
        .iter()
        .filter(|(_, symbol)| *symbol != 0 && value & symbol == *symbol)
        .collect();
    flags.sort_by_key(|(name, symbol)| (std::cmp::Reverse(symbol.count_ones()), *name));
    let mut covered = 0;
    let mut names = Vec::new();
    for (name, symbol) in flags {
        if covered & symbol != *symbol {
            covered |= symbol;
            names.push(*name);
        }
    }
    (covered == value && !names.is_empty()).then(|| names.join("|"))
}

static NAMES_BY_ID: LazyLock<Vec<(u32, &str, &str)>> = LazyLock::new(|| {
    let mut names: Vec<(u32, &str, &str)> = ANDROID_RESOURCES
        .iter()
        .map(|&((type_name, entry), id)| (id, type_name, entry))
        .chain(
            ANDROID_ATTRS
                .iter()
                .map(|(name, attr)| (attr.id, "attr", *name)),
        )
        .collect();
    names.sort_unstable();
    names.dedup_by_key(|&mut (id, ..)| id);
    names
});

/// The value public framework attribute `attr_id` gives the enum or flag name `symbol`.
pub fn android_attr_symbol(attr_id: u32, symbol: &str) -> Option<AttrSymbol> {
    ANDROID_ATTR_SYMBOLS
        .binary_search_by_key(&(attr_id, symbol), |(name, _)| *name)
        .ok()
        .map(|index| AttrSymbol {
            value: ANDROID_ATTR_SYMBOLS[index].1,
            flags: android_attr_formats(attr_id)
                .is_some_and(|formats| formats.contains(AttrFormats::FLAGS)),
        })
}

struct AttributeInfo {
    id: u32,
    formats: u32,
}

static FORMATS_BY_ID: LazyLock<Vec<(u32, AttrFormats)>> = LazyLock::new(|| {
    let mut formats = ANDROID_ATTRS
        .iter()
        .map(|(_, attr)| (attr.id, AttrFormats::from_bits_retain(attr.formats)))
        .collect::<Vec<_>>();
    formats.sort_unstable_by_key(|&(id, _)| id);
    formats
});

pub(crate) fn android_attr_formats(id: u32) -> Option<AttrFormats> {
    FORMATS_BY_ID
        .binary_search_by_key(&id, |&(id, _)| id)
        .ok()
        .map(|i| FORMATS_BY_ID[i].1)
}
