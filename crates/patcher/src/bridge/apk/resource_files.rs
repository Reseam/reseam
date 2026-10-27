// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_apk::Compression;
use reseam_apk::axml;

use super::components::{component, table, table_mut, with_component, with_scope};
use super::resources::defining_component;
use crate::bridge::phase::with_ctx_result;

#[export]
pub fn res_file_paths(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Vec<String>, String> {
    with_ctx_result(|ctx| {
        let index = defining_component(ctx, component_name.as_deref(), |apk| {
            Ok(apk
                .find_resource(&res_type, &res_name)?
                .map(|(index, _)| index))
        })?
        .unwrap_or(0);
        let paths = table(ctx, index)?
            .file_paths(&res_type, &res_name)
            .map_err(|error| error.to_string())?;
        let component = component(ctx, index);
        match paths.iter().find(|path| !component.contains(path)) {
            Some(missing) => Err(format!(
                "{res_type}/{res_name} is not file-backed: its value {missing} is not an entry of the APK"
            )),
            None => Ok(paths),
        }
    })
}

#[export]
pub fn res_file_path(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<String, String> {
    Ok(res_file_paths(component, res_type, res_name)?.swap_remove(0))
}

#[export]
pub fn res_add_file(
    component_name: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component_name, |ctx, index| {
        if !component(ctx, index).contains(&apk_path) {
            return Err(format!(
                "{res_type}/{name}: the APK has no entry {apk_path} to register"
            ));
        }
        table_mut(ctx, index)?
            .add_file_resource(&res_type, &name, &apk_path, &qualifiers)
            .map_err(|error| format!("{res_type}/{name}: {error}"))
    })
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Android resource entry suffixes are case-sensitive"
)]
#[export]
pub fn res_add_file_data(
    component_name: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    data: Vec<u8>,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component_name, |ctx, index| {
        if let Ok(text) = std::str::from_utf8(&data)
            && apk_path.ends_with(".xml")
            && !axml::is_compiled_axml(&data)
        {
            let files = with_scope(ctx, index, |scope| {
                axml::compile_resource_file(text, &apk_path, &res_type, &name, &qualifiers, scope)
                    .map_err(|error| format!("{res_type}/{name}: {error}"))
            })?;
            let id = files
                .last()
                .ok_or_else(|| format!("{res_type}/{name}: compiler produced no main file"))?
                .res_id;
            for file in files {
                ctx.inject_file(index, &file.path, file.data, Compression::Deflated)
                    .map_err(|error| error.to_string())?;
            }
            return Ok(id);
        }
        ctx.inject_file(index, &apk_path, data, Compression::Deflated)
            .map_err(|error| format!("{res_type}/{name}: {error}"))?;
        table_mut(ctx, index)?
            .add_file_resource(&res_type, &name, &apk_path, &qualifiers)
            .map_err(|error| format!("{res_type}/{name}: {error}"))
    })
}
