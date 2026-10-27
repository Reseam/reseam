// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Component, Path};

use boltffi::export;

use crate::bridge::phase::{bundle_path, with_output};
use crate::bridge::run_state::checked;
use crate::error::{PatcherError, Result};

#[export]
pub fn output_components() -> Vec<String> {
    with_output(|ctx| ctx.apk().component_names().map(str::to_owned).collect())
}

#[export]
pub fn output_path(component: Option<String>) -> String {
    with_output(|ctx| checked(ctx.apk().path(component.as_deref()).and_then(path_text)))
}

#[export]
pub fn output_replace(component: Option<String>, replacement: String) {
    with_output(|ctx| {
        checked(
            ctx.apk()
                .replace(component.as_deref(), Path::new(&replacement)),
        );
    });
}

#[export]
pub fn output_temp_file(name: String) -> String {
    with_output(|ctx| checked(ctx.apk().temp_file(&name).and_then(|path| path_text(&path))))
}

#[export]
pub fn bundle_file_path(bundle_relative: String) -> String {
    checked(resolve_bundle_file(&bundle_relative))
}

fn resolve_bundle_file(bundle_relative: &str) -> Result<String> {
    let relative = Path::new(bundle_relative);
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(PatcherError::InvalidFile(format!(
            "bundle path '{bundle_relative}' must stay inside the bundle"
        )));
    }
    let path = bundle_path(bundle_relative);
    if !path.is_file() {
        return Err(PatcherError::NotFound(format!(
            "bundle file {bundle_relative}"
        )));
    }
    path_text(&path)
}

fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| PatcherError::InvalidFile(format!("{} is not a UTF-8 path", path.display())))
}
