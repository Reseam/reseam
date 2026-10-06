// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{ResValue, ResourceScope, ResourceTable};

use super::files::{inject, with_component};
use super::handles::{bundle_path, record_failure, with_ctx};
use super::types::{ResourceRef, ResourceScalar, StyleItem};
use crate::context::PatchContext;
use reseam_apk::Compression;

fn with_resources<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceTable) -> R,
) -> Result<R, String> {
    with_resources_result(component, |resources| Ok(f(resources)))
}

fn with_resources_result<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    with_component(component, |ctx, index| resources_of(ctx, index, f))
        .unwrap_or_else(|| Err("unknown component".to_string()))
}

pub(super) fn with_scope_result<R>(
    component: Option<String>,
    f: impl FnOnce(&mut ResourceScope<'_>) -> Result<R, String>,
) -> Result<R, String> {
    with_component(component, |ctx, index| scope_of(ctx, index, f))
        .unwrap_or_else(|| Err("unknown component".to_string()))
}

pub(super) fn with_resources_read_result<R>(
    component: Option<String>,
    f: impl FnOnce(&ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    with_component(component, |ctx, index| resources_of_read(ctx, index, f))
        .unwrap_or_else(|| Err("unknown component".to_string()))
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

#[export]
pub fn res_component_for(res_type: String, res_name: String) -> Result<Option<String>, String> {
    crate::kotlin::handles::with_ctx_result(|ctx| {
        let found = ctx
            .apk_mut()
            .find_resource(&res_type, &res_name)
            .map_err(|error| error.to_string())?;
        Ok(found.and_then(|(index, _)| ctx.apk().component(index).map(|c| c.name().to_string())))
    })
}

#[export]
pub fn res_component_for_id(res_id: u32) -> Result<Option<String>, String> {
    crate::kotlin::handles::with_ctx_result(|ctx| {
        let found = ctx
            .apk_mut()
            .find_resource_by_id(res_id)
            .map_err(|error| error.to_string())?;
        Ok(found.and_then(|index| ctx.apk().component(index).map(|c| c.name().to_string())))
    })
}

#[export]
pub fn res_id(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<u32>, String> {
    match component {
        None => crate::kotlin::handles::with_ctx_result(|ctx| {
            ctx.apk_mut()
                .find_resource(&res_type, &res_name)
                .map_err(|error| error.to_string())
                .map(|found| found.map(|(_, id)| id))
        }),
        Some(_) => with_resources_read_result(component, |resources| {
            resources
                .find_resource_id(&res_type, &res_name)
                .map_err(|error| error.to_string())
        }),
    }
}

#[export]
pub fn res_exists(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<bool, String> {
    Ok(res_id(component, res_type, res_name)?.is_some())
}

#[export]
pub fn res_get_string(component: Option<String>, name: String) -> Result<Option<String>, String> {
    match component {
        None => crate::kotlin::handles::with_ctx_result(|ctx| {
            ctx.apk_mut()
                .string_resource(&name)
                .map_err(|error| error.to_string())
        }),
        Some(_) => with_resources_read_result(component, |resources| {
            resources
                .string_value(&name)
                .map(|value| value.map(Cow::into_owned))
                .map_err(|error| error.to_string())
        }),
    }
}

#[export]
pub fn res_set_string(
    component: Option<String>,
    name: String,
    value: String,
) -> Result<bool, String> {
    match component {
        None => crate::kotlin::handles::with_ctx_result(|ctx| {
            ctx.apk_mut()
                .set_string_resource(&name, &value)
                .map_err(|error| error.to_string())
        }),
        Some(_) => with_resources_result(component, |resources| {
            resources
                .set_string_value(&name, &value)
                .map_err(|error| error.to_string())
        }),
    }
}

#[export]
pub fn res_add(
    component: Option<String>,
    res_type: String,
    name: String,
    value: String,
) -> Result<Option<u32>, String> {
    with_scope_result(component, |res| {
        if res_type == "string" {
            return res
                .table_mut()
                .add_string_resource(&name, &value)
                .map_err(|error| error.to_string());
        }
        match axml::infer_value(&value, Some(res)) {
            Ok(AttributeValue::Value(parsed)) => res
                .table_mut()
                .add_resource(&res_type, &name, parsed)
                .map_err(|error| error.to_string()),
            Ok(AttributeValue::Text) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    })
}

#[export]
pub fn res_add_id(component: Option<String>, name: String) -> Result<Option<u32>, String> {
    with_scope_result(component, |res| {
        res.ensure_id(&name).map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_add_raw(
    component: Option<String>,
    res_type: String,
    name: String,
    data_type: u8,
    data: u32,
) -> Result<Option<u32>, String> {
    with_resources_result(component, |res| {
        res.add_resource(&res_type, &name, ResValue::new(data_type, data))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_get_raw(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<i64>, String> {
    let component = match component {
        Some(component) => component,
        None => match res_component_for(res_type.clone(), res_name.clone())? {
            Some(component) => component,
            None => return Ok(None),
        },
    };
    with_resources_read_result(Some(component), |resources| {
        resources
            .resource_value(&res_type, &res_name)
            .map(|value| value.map(|value| i64::from(value.data)))
            .map_err(|error| error.to_string())
    })
}

pub(super) fn resources_of<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&mut ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    match ctx
        .component_mut(index)
        .and_then(|c| Ok(c.resources_mut()?))
    {
        Ok(Some(resources)) => f(resources),
        Ok(None) => Err("the component has no resource table".to_string()),
        Err(error) => Err(format!("resources: {error}")),
    }
}

pub(super) fn scope_of<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&mut ResourceScope<'_>) -> Result<R, String>,
) -> Result<R, String> {
    match ctx
        .apk_mut()
        .with_resource_scope(index, |scope| scope.map(f))
    {
        Ok(Some(result)) => result,
        Ok(None) => Err("the component has no resource table".to_string()),
        Err(error) => Err(format!("resources: {error}")),
    }
}

pub(super) fn resources_of_read<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&ResourceTable) -> Result<R, String>,
) -> Result<R, String> {
    match ctx.component_mut(index).and_then(|c| Ok(c.resources()?)) {
        Ok(Some(resources)) => f(resources),
        Ok(None) => Err("the component has no resource table".to_string()),
        Err(error) => Err(format!("resources: {error}")),
    }
}

#[export]
pub fn res_set_package_name(component: Option<String>, name: String) -> Result<(), String> {
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_package_name(&name)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_style_set(
    component: Option<String>,
    name: String,
    parent: Option<String>,
    items: Vec<StyleItem>,
) -> Result<u32, String> {
    let items: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.name, item.value))
        .collect();
    with_scope_result(component, |scope| {
        scope
            .set_style_items(&name, parent.as_deref(), &items)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_array_get(component: Option<String>, name: String) -> Result<Vec<String>, String> {
    with_component(component, |ctx, index| {
        resources_of_read(ctx, index, |resources| {
            resources.array(&name).map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_array_values_get(
    component: Option<String>,
    name: String,
) -> Result<Vec<ResourceScalar>, String> {
    with_resources_read_result(component, |resources| {
        resources
            .array_values(&name)
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| ResourceScalar {
                        kind: value.kind,
                        data: value.data,
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_array_values_set(
    component: Option<String>,
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
    with_resources_result(component, |resources| {
        resources
            .set_array_values(&name, &values)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_array_set(
    component: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_scope_result(component, |scope| {
        scope
            .set_array(&name, &values)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_string_array_set(
    component: Option<String>,
    name: String,
    values: Vec<String>,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        resources_of(ctx, index, |resources| {
            resources
                .set_string_array(&name, &values)
                .map_err(|error| error.to_string())
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[export]
pub fn res_copy(bundle_relative: String, apk_path: String) {
    let source = bundle_path(&bundle_relative);
    match std::fs::read(&source) {
        Ok(data) => inject(None, &apk_path, data, Compression::Deflated),
        Err(error) => with_ctx(|ctx| {
            record_failure(format!(
                "read bundle resource {}: {error}",
                source.display()
            ));
            ctx.log().warn(format!(
                "res_copy: failed to read {}: {error}",
                source.display()
            ));
        }),
    }
}

#[export]
pub fn res_copy_group(res_type: String, files: Vec<String>) {
    let bundle_dir = bundle_path("");
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    with_ctx(|ctx| {
        if let Err(error) = ctx.copy_resource_group(&bundle_dir, &res_type, &files) {
            record_failure(&error);
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
pub fn res_pool_get(component: Option<String>, index: u32) -> Result<Option<String>, String> {
    with_resources_read_result(component, |res| {
        res.get_string(index)
            .map(|value| value.map(Cow::into_owned))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_pool_set(component: Option<String>, index: u32, value: String) -> Result<(), String> {
    with_resources_result(component, |res| {
        res.set_string(index, value)
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_pool_add(component: Option<String>, value: String) -> Result<u32, String> {
    with_resources(component, |res| res.add_global_string(&value))
}

#[export]
pub fn res_pool_find_refs(
    component: Option<String>,
    string_index: u32,
) -> Result<Vec<ResourceRef>, String> {
    with_resources_read_result(component, |res| {
        Ok(res
            .find_entries_by_string(string_index)
            .map_err(|error| error.to_string())?
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
    component: Option<String>,
    res_id: u32,
    new_string_index: u32,
) -> Result<(), String> {
    let component = match component {
        Some(component) => component,
        None => res_component_for_id(res_id)?
            .ok_or_else(|| format!("no component defines resource 0x{res_id:08x}"))?,
    };
    with_resources_result(Some(component), |res| {
        res.replace_entry_string(res_id, new_string_index)
            .map_err(|error| error.to_string())
    })
}
