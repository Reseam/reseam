// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_apk::Compression;
use reseam_apk::axml;
use reseam_apk::resources::{ResourceFile, ResourceName};

use super::components::{component, table, table_mut, with_component, with_scope};
use super::resources::defining_component;
use crate::bridge::dex::types::ConfigFile;
use crate::bridge::phase::with_ctx_result;

fn resource_files(
    component_name: Option<&str>,
    res_type: &str,
    res_name: &str,
) -> Result<Vec<ResourceFile>, String> {
    with_ctx_result(|ctx| {
        let index = defining_component(ctx, component_name, |apk| {
            Ok(apk
                .find_resource(res_type, res_name)?
                .map(|(index, _)| index))
        })?
        .unwrap_or(0);
        let files = table(ctx, index)?
            .files(res_type, res_name)
            .map_err(|error| error.to_string())?;
        let component = component(ctx, index);
        match files.iter().find(|file| !component.contains(&file.path)) {
            Some(missing) => Err(format!(
                "{res_type}/{res_name} is not file-backed: its value {} is not an entry of the APK",
                missing.path
            )),
            None => Ok(files),
        }
    })
}

#[export]
pub fn res_file_paths(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Vec<String>, String> {
    Ok(
        resource_files(component_name.as_deref(), &res_type, &res_name)?
            .into_iter()
            .map(|file| file.path)
            .collect(),
    )
}

#[export]
pub fn res_files(
    component_name: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Vec<ConfigFile>, String> {
    Ok(
        resource_files(component_name.as_deref(), &res_type, &res_name)?
            .into_iter()
            .map(|ResourceFile { qualifiers, path }| ConfigFile { qualifiers, path })
            .collect(),
    )
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
pub fn res_file_path_by_id(component: Option<String>, res_id: u32) -> Result<String, String> {
    let name = reference_name(component.clone(), res_id)?
        .ok_or_else(|| format!("no resource has the id {res_id:#010x}"))?;
    res_file_path(component, name.type_name, name.entry)
}

#[export]
pub fn res_reference_by_id(
    component: Option<String>,
    res_id: u32,
) -> Result<Option<String>, String> {
    Ok(reference_name(component, res_id)?.map(|name| name.reference('@')))
}

fn reference_name(component: Option<String>, res_id: u32) -> Result<Option<ResourceName>, String> {
    with_component(component, |ctx, index| {
        table(ctx, index)?
            .reference_name(res_id)
            .map_err(|error| error.to_string())
    })
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
