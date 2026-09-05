// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `resources.arsc` entries and the global string pool. `component` is a
//! split name; `None` means the base.

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{ResValue, ResourceTable};

use super::files::{inject, with_component};
use super::handles::{bundle_path, with_ctx};
use super::types::ResourceRef;
use reseam_apk::Compression;

/// Runs `f` on the named component's resource table, logging when the
/// component has none or its table cannot be read.
fn with_resources<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceTable) -> R,
) -> Option<R> {
    with_component(component, |ctx, index| {
        match ctx
            .component_mut(index)
            .and_then(|c| Ok(c.resources_mut()?))
        {
            Ok(Some(resources)) => Some(f(resources)),
            Ok(None) => None,
            Err(error) => {
                ctx.log().warn(format!("resources: {error}"));
                None
            }
        }
    })
    .flatten()
}

#[export]
pub fn res_component_names() -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .components()
            .iter()
            .filter(|c| c.has_resources())
            .map(|c| c.name().to_string())
            .collect()
    })
}

/// The component defining `res_type/res_name`, searching all of them.
#[export]
pub fn res_component_for(res_type: String, res_name: String) -> Option<String> {
    with_ctx(|ctx| {
        let (index, _) = ctx
            .apk_mut()
            .find_resource(&res_type, &res_name)
            .ok()
            .flatten()?;
        Some(ctx.apk().component(index)?.name().to_string())
    })
}

#[export]
pub fn res_component_for_id(res_id: u32) -> Option<String> {
    with_ctx(|ctx| {
        let index = ctx.apk_mut().find_resource_by_id(res_id).ok().flatten()?;
        Some(ctx.apk().component(index)?.name().to_string())
    })
}

/// The id of `res_type/res_name`; without a component every one is searched.
#[export]
pub fn res_id(component: Option<String>, res_type: String, res_name: String) -> Option<u32> {
    match component {
        None => with_ctx(|ctx| {
            ctx.apk_mut()
                .find_resource(&res_type, &res_name)
                .ok()
                .flatten()
                .map(|(_, id)| id)
        }),
        Some(_) => {
            with_resources(component, |res| res.find_resource_id(&res_type, &res_name)).flatten()
        }
    }
}

#[export]
pub fn res_exists(component: Option<String>, res_type: String, res_name: String) -> bool {
    res_id(component, res_type, res_name).is_some()
}

#[export]
pub fn res_get_string(component: Option<String>, name: String) -> Option<String> {
    match component {
        None => with_ctx(|ctx| ctx.apk_mut().string_resource(&name).ok().flatten()),
        Some(_) => with_resources(component, |res| {
            res.string_value(&name).map(|s| s.into_owned())
        })
        .flatten(),
    }
}

#[export]
pub fn res_set_string(component: Option<String>, name: String, value: String) -> bool {
    match component {
        None => with_ctx(|ctx| {
            ctx.apk_mut()
                .set_string_resource(&name, &value)
                .unwrap_or(false)
        }),
        Some(_) => {
            with_resources(component, |res| res.set_string_value(&name, &value)).unwrap_or(false)
        }
    }
}

/// Adds `res_type/name` with `value` read the way resource XML is: booleans,
/// integers, colors, dimensions and `@type/name` references. A `string`
/// entry keeps the text as is.
#[export]
pub fn res_add(
    component: Option<String>,
    res_type: String,
    name: String,
    value: String,
) -> Option<u32> {
    with_resources(component, |res| {
        if res_type == "string" {
            return res.add_string_resource(&name, &value);
        }
        match axml::parse_attribute_value(&value, Some(res)) {
            Ok(AttributeValue::Value(parsed)) => res.add_resource(&res_type, &name, parsed),
            _ => None,
        }
    })
    .flatten()
}

#[export]
pub fn res_add_id(component: Option<String>, name: String) -> Option<u32> {
    with_resources(component, |res| res.ensure_id(&name)).flatten()
}

#[export]
pub fn res_add_raw(
    component: Option<String>,
    res_type: String,
    name: String,
    data_type: u8,
    data: u32,
) -> Option<u32> {
    with_resources(component, |res| {
        res.add_resource(&res_type, &name, ResValue::new(data_type, data))
    })
    .flatten()
}

#[export]
pub fn res_get_raw(component: Option<String>, res_type: String, res_name: String) -> Option<i64> {
    let component = component.or_else(|| res_component_for(res_type.clone(), res_name.clone()))?;
    with_resources(Some(component), |res| {
        res.resource_value(&res_type, &res_name)
            .map(|v| v.data as i64)
    })
    .flatten()
}

#[export]
pub fn res_copy(bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(None, &apk_path, data, Compression::Deflated),
        Err(error) => with_ctx(|ctx| {
            ctx.log().warn(format!(
                "res_copy: failed to read {}: {error}",
                source.display()
            ))
        }),
    }
}

/// Copies `resources/<res_type>/<file>` from the bundle into `res/<res_type>/`.
#[export]
pub fn res_copy_group(res_type: String, files: Vec<String>) {
    let bundle_dir = bundle_path("");
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    with_ctx(|ctx| {
        if let Err(error) = ctx.copy_resource_group(&bundle_dir, &res_type, &files) {
            ctx.log().warn(format!("res_copy_group: {error}"));
        }
    });
}

#[export]
pub fn res_inject(apk_path: String, data: Vec<u8>) {
    inject(None, &apk_path, data, Compression::Deflated);
}

#[export]
pub fn res_delete(apk_path: String) {
    super::files::file_delete(None, apk_path);
}

#[export]
pub fn res_list(prefix: String) -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .entry_names()
            .iter()
            .filter(|name| name.as_str().starts_with(&prefix))
            .map(ToString::to_string)
            .collect()
    })
}

#[export]
pub fn res_pool_get(component: Option<String>, index: u32) -> Option<String> {
    with_resources(component, |res| {
        res.get_string(index).map(|s| s.into_owned())
    })
    .flatten()
}

#[export]
pub fn res_pool_set(component: Option<String>, index: u32, value: String) {
    with_resources(component, |res| res.set_string(index, value));
}

#[export]
pub fn res_pool_add(component: Option<String>, value: String) -> Option<u32> {
    with_resources(component, |res| res.add_global_string(&value))
}

#[export]
pub fn res_pool_find_refs(component: Option<String>, string_index: u32) -> Vec<ResourceRef> {
    with_resources(component, |res| {
        res.find_entries_by_string(string_index)
            .into_iter()
            .map(|entry| ResourceRef {
                res_id: entry.res_id,
                key_name: entry.key_name,
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Points a string entry at another pool string; without a component the
/// entry's own component is used.
#[export]
pub fn res_replace_entry(component: Option<String>, res_id: u32, new_string_index: u32) {
    let Some(component) = component.or_else(|| res_component_for_id(res_id)) else {
        return;
    };
    with_resources(Some(component), |res| {
        res.replace_entry_string(res_id, new_string_index)
    });
}
