// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Resource references in attribute text: `@type/name`, `@+id/name`,
//! `?attr/name`, their `android:` forms, and literal `@0x` ids.

use super::android_attrs::android_res_id;
use crate::error::{Result, invalid};
use crate::resources::{ResourceScope, ResourceTable};

fn hex_ref(text: &str) -> Option<u32> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("ref/0x"))?;
    u32::from_str_radix(hex, 16).ok()
}

pub(super) fn attribute_ref(text: &str, resources: Option<&ResourceTable>) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
    if let Some(name) = text.strip_prefix("android:attr/") {
        return android_resource("attr", name).map(Some);
    }
    let name = text.strip_prefix("attr/").unwrap_or(text);
    Ok(resources
        .map(|res| res.find_resource_id("attr", name))
        .transpose()?
        .flatten())
}

pub(super) fn resource_ref(
    text: &str,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
    let create = text.starts_with("+id/");
    let text = text.strip_prefix('+').unwrap_or(text);
    let Some((type_part, entry)) = text.split_once('/') else {
        return Ok(None);
    };
    let (namespace, type_name) = match type_part.split_once(':') {
        Some((namespace, type_name)) => (Some(namespace), type_name),
        None => (None, type_part),
    };
    if type_name.is_empty() || entry.is_empty() {
        return Ok(None);
    }
    Ok(match (namespace, resources) {
        (Some("android"), _) => Some(android_resource(type_name, entry)?),
        (Some(_), _) | (None, None) => None,
        (None, Some(res)) if create => res.ensure_id(entry)?,
        // aapt fails the build here; plain text would reach the inflater as a string and crash it.
        (None, Some(res)) => Some(res.resource_id(type_name, entry)?.ok_or_else(|| {
            invalid(
                "axml compiler",
                format!("@{type_name}/{entry} is not defined in the resource table"),
            )
        })?),
    })
}

/// The id of a public framework resource, which a missing name cannot fall back from.
pub(super) fn android_resource(type_name: &str, name: &str) -> Result<u32> {
    android_res_id(type_name, name).ok_or_else(|| {
        invalid(
            "framework resource",
            format!("android:{type_name}/{name} is not public"),
        )
    })
}
