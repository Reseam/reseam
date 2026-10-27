// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Styles, arrays and the global string pool of a component's resource table.

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::ResValue;

use super::components::{table, table_mut, with_component, with_scope};
use super::resources::defining_component;
use crate::bridge::dex::types::{ResourceRef, ResourceScalar, StyleItem};
use crate::bridge::phase::with_ctx_result;

#[export]
pub fn res_style_set(
    component_name: Option<String>,
    name: String,
    parent: Option<String>,
    items: Vec<StyleItem>,
) -> Result<u32, String> {
    let items: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.name, item.value))
        .collect();
    with_component(component_name, |ctx, index| {
        with_scope(ctx, index, |scope| {
            scope
                .set_style_items(&name, parent.as_deref(), &items)
                .map_err(|error| error.to_string())
        })
    })
}

#[export]
pub fn res_array_get(component_name: Option<String>, name: String) -> Result<Vec<String>, String> {
    with_component(component_name, |ctx, index| {
        table(ctx, index)?
            .array(&name)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_array_values_get(
    component_name: Option<String>,
    name: String,
) -> Result<Vec<ResourceScalar>, String> {
    with_component(component_name, |ctx, index| {
        let values = table(ctx, index)?
            .array_values(&name)
            .map_err(|error| error.to_string())?;
        Ok(values
            .into_iter()
            .map(|value| ResourceScalar {
                kind: value.kind,
                data: value.data,
            })
            .collect())
    })
}

#[export]
pub fn res_array_values_set(
    component_name: Option<String>,
    name: String,
    values: Vec<ResourceScalar>,
) -> Result<u32, String> {
    let values: Vec<_> = values
        .into_iter()
        .map(|value| ResValue {
            kind: value.kind,
            data: value.data,
        })
        .collect();
    with_component(component_name, |ctx, index| {
        table_mut(ctx, index)?
            .set_array_values(&name, &values)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_array_set(
    component_name: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_component(component_name, |ctx, index| {
        with_scope(ctx, index, |scope| {
            scope
                .set_array(&name, &values)
                .map_err(|error| error.to_string())
        })
    })
}

#[export]
pub fn res_string_array_set(
    component_name: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_component(component_name, |ctx, index| {
        table_mut(ctx, index)?
            .set_string_array(&name, &values)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_pool_get(component_name: Option<String>, index: u32) -> Result<Option<String>, String> {
    with_component(component_name, |ctx, component| {
        table(ctx, component)?
            .get_string(index)
            .map(|value| value.map(Cow::into_owned))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_pool_set(
    component_name: Option<String>,
    index: u32,
    value: String,
) -> Result<(), String> {
    with_component(component_name, |ctx, component| {
        table_mut(ctx, component)?
            .set_string(index, value)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_pool_add(component_name: Option<String>, value: String) -> Result<u32, String> {
    with_component(component_name, |ctx, index| {
        Ok(table_mut(ctx, index)?.add_global_string(&value))
    })
}

#[export]
pub fn res_pool_find_refs(
    component_name: Option<String>,
    string_index: u32,
) -> Result<Vec<ResourceRef>, String> {
    with_component(component_name, |ctx, index| {
        let entries = table(ctx, index)?
            .find_entries_by_string(string_index)
            .map_err(|error| error.to_string())?;
        Ok(entries
            .into_iter()
            .map(|entry| ResourceRef {
                res_id: entry.res_id,
                key_name: entry.key_name,
            })
            .collect())
    })
}

#[export]
pub fn res_replace_entry(
    component_name: Option<String>,
    res_id: u32,
    new_string_index: u32,
) -> Result<(), String> {
    with_ctx_result(|ctx| {
        let index = defining_component(ctx, component_name.as_deref(), |apk| {
            apk.find_resource_by_id(res_id)
        })?
        .ok_or_else(|| format!("no component defines resource 0x{res_id:08x}"))?;
        table_mut(ctx, index)?
            .replace_entry_string(res_id, new_string_index)
            .map_err(|error| error.to_string())
    })
}
