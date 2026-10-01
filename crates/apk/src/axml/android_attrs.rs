// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::LazyLock;

use crate::resources::{AttrFormats, AttrSymbol};

mod data;

use data::{ANDROID_ATTR_SYMBOLS, ANDROID_ATTRS, ANDROID_RESOURCES};
pub use data::{
    ATTR_CONFIG_CHANGES, ATTR_DRAWABLE, ATTR_ENABLED, ATTR_ICON, ATTR_LABEL, ATTR_MIME_TYPE,
    ATTR_MIN_SDK_VERSION, ATTR_NAME, ATTR_SPLIT_NAME, ATTR_TARGET_ACTIVITY, ATTR_VERSION_CODE,
    ATTR_VERSION_NAME,
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
