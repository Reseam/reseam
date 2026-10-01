// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::files::with_component;
use boltffi::export;
use reseam_apk::Compression;
use reseam_apk::axml::{self};

use super::resources::{res_component_for, resources_of, resources_of_read, scope_of};

#[export]
pub fn res_file_paths(
    component: Option<String>,
    res_type: String,
    res_name: String,
) -> Result<Vec<String>, String> {
    let component = match component {
        Some(component) => Some(component),
        None => res_component_for(res_type.clone(), res_name.clone())?,
    };
    with_component(component, |ctx, index| {
        let paths = resources_of_read(ctx, index, |resources| {
            resources
                .file_paths(&res_type, &res_name)
                .map_err(|error| error.to_string())
        })?;
        let component = ctx.apk().component(index).ok_or("unknown component")?;
        match paths.iter().find(|path| !component.contains(path)) {
            Some(missing) => Err(format!(
                "{res_type}/{res_name} is not file-backed: its value {missing} is not an entry of the APK"
            )),
            None => Ok(paths),
        }
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
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
    component: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        if !ctx
            .apk()
            .component(index)
            .is_some_and(|component| component.contains(&apk_path))
        {
            return Err(format!(
                "{res_type}/{name}: the APK has no entry {apk_path} to register"
            ));
        }
        resources_of(ctx, index, |resources| {
            resources
                .add_file_resource(&res_type, &name, &apk_path, &qualifiers)
                .map_err(|error| format!("{res_type}/{name}: {error}"))
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Android resource entry suffixes are case-sensitive"
)]
#[export]
pub fn res_add_file_data(
    component: Option<String>,
    res_type: String,
    name: String,
    apk_path: String,
    data: Vec<u8>,
    qualifiers: String,
) -> Result<u32, String> {
    with_component(component, |ctx, index| {
        if let Ok(text) = std::str::from_utf8(&data)
            && apk_path.ends_with(".xml")
            && !axml::is_compiled_axml(&data)
        {
            let files = scope_of(ctx, index, |scope| {
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
        resources_of(ctx, index, |resources| {
            resources
                .add_file_resource(&res_type, &name, &apk_path, &qualifiers)
                .map_err(|error| format!("{res_type}/{name}: {error}"))
        })
    })
    .unwrap_or_else(|| Err("unknown component".to_string()))
}
