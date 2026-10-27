// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The APK component an export names, and its resource table.

use boltffi::export;
use reseam_apk::{ApkComponent, ResourceScope, ResourceTable};

use crate::bridge::phase::{with_ctx, with_ctx_result};
use crate::context::PatchContext;

/// The index of the component `name` names; the base when `None`.
pub(in crate::bridge) fn component_index(
    ctx: &PatchContext<'_>,
    name: Option<&str>,
) -> Result<usize, String> {
    match name {
        None => Ok(0),
        Some(name) => ctx
            .apk()
            .component_by_name(name)
            .ok_or_else(|| format!("unknown component {name}")),
    }
}

/// Runs `f` with the index of the component `name` names.
pub(in crate::bridge) fn with_component<R>(
    name: Option<String>,
    f: impl FnOnce(&mut PatchContext<'_>, usize) -> Result<R, String>,
) -> Result<R, String> {
    with_ctx_result(|ctx| {
        let index = component_index(ctx, name.as_deref())?;
        f(ctx, index)
    })
}

pub(in crate::bridge) fn component<'c>(
    ctx: &'c PatchContext<'_>,
    index: usize,
) -> &'c ApkComponent {
    ctx.apk()
        .component(index)
        .expect("component index comes from component_index")
}

pub(in crate::bridge) fn table<'c>(
    ctx: &'c mut PatchContext<'_>,
    index: usize,
) -> Result<&'c ResourceTable, String> {
    ctx.component_mut(index)
        .and_then(|component| Ok(component.resources()?))
        .map_err(|error| format!("resources: {error}"))?
        .ok_or_else(no_table)
}

pub(in crate::bridge) fn table_mut<'c>(
    ctx: &'c mut PatchContext<'_>,
    index: usize,
) -> Result<&'c mut ResourceTable, String> {
    ctx.component_mut(index)
        .and_then(|component| Ok(component.resources_mut()?))
        .map_err(|error| format!("resources: {error}"))?
        .ok_or_else(no_table)
}

/// Runs `f` in the resource scope of component `index`: its table, with the
/// base table for references it does not define.
pub(in crate::bridge) fn with_scope<R>(
    ctx: &mut PatchContext<'_>,
    index: usize,
    f: impl FnOnce(&mut ResourceScope<'_>) -> Result<R, String>,
) -> Result<R, String> {
    ctx.apk_mut()
        .with_resource_scope(index, |scope| scope.map(f))
        .map_err(|error| format!("resources: {error}"))?
        .ok_or_else(no_table)?
}

fn no_table() -> String {
    "the component has no resource table".to_owned()
}

#[export]
pub fn component_names() -> Vec<String> {
    with_ctx(|ctx| {
        ctx.apk()
            .components()
            .iter()
            .map(|c| c.name().to_string())
            .collect()
    })
}
