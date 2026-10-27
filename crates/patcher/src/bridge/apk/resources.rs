// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue};
use reseam_apk::{ApkFile, ResValue};

use super::components::{component, table, table_mut, with_component, with_scope};
use crate::bridge::phase::{with_ctx, with_ctx_result};
use crate::context::PatchContext;

/// Runs `app` across every component's table when no component is named, as
/// the platform merges them, and `named` on the named component otherwise.
fn app_or_component<R>(
    component_name: Option<String>,
    app: impl FnOnce(&mut ApkFile) -> reseam_apk::Result<R>,
    named: impl FnOnce(&mut PatchContext<'_>, usize) -> Result<R, String>,
) -> Result<R, String> {
    match component_name {
        None => with_ctx_result(|ctx| app(ctx.apk_mut()).map_err(|error| error.to_string())),
        Some(_) => with_component(component_name, named),
    }
}

/// The index of the named component, or else of the component that defines
/// the resource `find` looks for.
pub(in crate::bridge) fn defining_component(
    ctx: &mut PatchContext<'_>,
    component_name: Option<&str>,
    find: impl FnOnce(&mut ApkFile) -> reseam_apk::Result<Option<usize>>,
) -> Result<Option<usize>, String> {
    match component_name {
        Some(_) => super::components::component_index(ctx, component_name).map(Some),
        None => find(ctx.apk_mut()).map_err(|error| error.to_string()),
    }
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
    component_name(|apk| {
        Ok(apk
            .find_resource(&res_type, &res_name)?
            .map(|(index, _)| index))
    })
}

#[export]
pub fn res_component_for_id(res_id: u32) -> Result<Option<String>, String> {
    component_name(|apk| apk.find_resource_by_id(res_id))
}

fn component_name(
    find: impl FnOnce(&mut ApkFile) -> reseam_apk::Result<Option<usize>>,
) -> Result<Option<String>, String> {
    with_ctx_result(|ctx| {
        Ok(defining_component(ctx, None, find)?
            .map(|index| component(ctx, index).name().to_string()))
    })
}

#[export]
pub fn res_id(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<u32>, String> {
    app_or_component(
        component_name,
        |apk| Ok(apk.find_resource(&res_type, &res_name)?.map(|(_, id)| id)),
        |ctx, index| {
            table(ctx, index)?
                .find_resource_id(&res_type, &res_name)
                .map_err(|error| error.to_string())
        },
    )
}

#[export]
pub fn res_exists(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<bool, String> {
    Ok(res_id(component_name, res_type, res_name)?.is_some())
}

#[export]
pub fn res_get_string(
    component_name: Option<String>,
    name: String,
) -> Result<Option<String>, String> {
    app_or_component(
        component_name,
        |apk| apk.string_resource(&name),
        |ctx, index| {
            table(ctx, index)?
                .string_value(&name)
                .map(|value| value.map(Cow::into_owned))
                .map_err(|error| error.to_string())
        },
    )
}

#[export]
pub fn res_set_string(
    component_name: Option<String>,
    name: String,
    value: String,
) -> Result<bool, String> {
    app_or_component(
        component_name,
        |apk| apk.set_string_resource(&name, &value),
        |ctx, index| {
            table_mut(ctx, index)?
                .set_string_value(&name, &value)
                .map_err(|error| error.to_string())
        },
    )
}

#[export]
pub fn res_get_string_by_id(
    component_name: Option<String>,
    res_id: u32,
) -> Result<Option<String>, String> {
    app_or_component(
        component_name,
        |apk| apk.string_resource_by_id(res_id),
        |ctx, index| {
            table(ctx, index)?
                .string_value_by_id(res_id)
                .map(|value| value.map(Cow::into_owned))
                .map_err(|error| error.to_string())
        },
    )
}

#[export]
pub fn res_set_string_by_id(
    component_name: Option<String>,
    res_id: u32,
    value: String,
) -> Result<bool, String> {
    app_or_component(
        component_name,
        |apk| apk.set_string_resource_by_id(res_id, &value),
        |ctx, index| {
            table_mut(ctx, index)?
                .set_string_value_by_id(res_id, &value)
                .map_err(|error| error.to_string())
        },
    )
}

/// Adds resource `name` in the configuration `qualifiers` names, written as in a `res/` directory name.
#[export]
pub fn res_add_in(
    component_name: Option<String>,
    res_type: String,
    name: String,
    value: String,
    qualifiers: String,
) -> Result<Option<u32>, String> {
    with_component(component_name, |ctx, index| {
        with_scope(ctx, index, |scope| {
            let parsed = if res_type == "string" {
                ResValue::string(scope.table_mut().add_global_string(&value))
            } else {
                match axml::parse_value(&value, None, Some(scope)) {
                    Ok(AttributeValue::Value(parsed)) => parsed,
                    Ok(AttributeValue::Text) => return Ok(None),
                    Err(error) => return Err(error.to_string()),
                }
            };
            scope
                .table_mut()
                .add_resource_in(&res_type, &name, parsed, &qualifiers)
                .map_err(|error| error.to_string())
        })
    })
}

#[export]
pub fn res_add_id(component_name: Option<String>, name: String) -> Result<Option<u32>, String> {
    with_component(component_name, |ctx, index| {
        with_scope(ctx, index, |scope| {
            scope.ensure_id(&name).map_err(|error| error.to_string())
        })
    })
}

#[export]
pub fn res_add_raw(
    component_name: Option<String>,
    res_type: String,
    name: String,
    data_type: u8,
    data: u32,
) -> Result<Option<u32>, String> {
    with_component(component_name, |ctx, index| {
        table_mut(ctx, index)?
            .add_resource(&res_type, &name, ResValue::new(data_type, data))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_get_raw(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Option<i64>, String> {
    with_ctx_result(|ctx| {
        let found = defining_component(ctx, component_name.as_deref(), |apk| {
            Ok(apk
                .find_resource(&res_type, &res_name)?
                .map(|(index, _)| index))
        })?;
        let Some(index) = found else {
            return Ok(None);
        };
        table(ctx, index)?
            .resource_value(&res_type, &res_name)
            .map(|value| value.map(|value| i64::from(value.data)))
            .map_err(|error| error.to_string())
    })
}

#[export]
pub fn res_set_package_name(component_name: Option<String>, name: String) -> Result<(), String> {
    with_component(component_name, |ctx, index| {
        table_mut(ctx, index)?
            .set_package_name(&name)
            .map_err(|error| error.to_string())
    })
}
