// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A new bundle project made from the patch bundle template.

use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use reseam_patcher::bundle::{BundleInfo, ENGINE_VERSION, manifest_info, write_manifest_info};
use zip::ZipArchive;

use crate::error::{HostError, Result, file_error};

/// The newest bundle template, as the ZIP archive [`init_bundle`] takes.
pub const TEMPLATE_URL: &str =
    "https://git.reseam.app/api/v1/repos/reseam/patches-template/archive/main.zip";

const SETTINGS: &str = "settings.gradle.kts";
const PLUGIN: &str = "id(\"app.reseam.workspace\") version ";
const ROOT_PROJECT: &str = "rootProject.name = ";

/// Makes `dir` a bundle project named `name` from `template`, a ZIP archive of
/// the bundle template repository with its files in one top-level folder. The
/// project builds with this engine's Gradle plugin. Fails when `dir` exists or
/// `name` is not a bundle name; nothing is left behind then.
pub fn init_bundle(template: impl Read + Seek, name: &str, dir: &Path) -> Result<()> {
    if dir.exists() {
        return Err(HostError::Exists(dir.to_owned()));
    }
    let parent = match dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent).map_err(|error| file_error("create", parent, error))?;
    let staging = tempfile::Builder::new()
        .prefix(".bundle-init-")
        .tempdir_in(parent)
        .map_err(|error| file_error("create a folder in", parent, error))?;
    extract(template, staging.path())?;
    let manifest = staging.path().join("manifest.toml");
    write_manifest_info(
        &manifest,
        BundleInfo {
            name: name.to_owned(),
            ..manifest_info(&manifest)?
        },
    )?;
    set_settings(&staging.path().join(SETTINGS), name)?;
    let staged = staging.keep();
    fs::rename(&staged, dir).map_err(|error| file_error("move the project to", dir, error))
}

/// Writes the archive's files under `to`, without their top-level folder.
fn extract(template: impl Read + Seek, to: &Path) -> Result<()> {
    let mut archive = ZipArchive::new(template)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let relative: PathBuf = entry
            .enclosed_name()
            .ok_or_else(|| HostError::TemplatePath(entry.name().to_owned()))?
            .components()
            .skip(1)
            .collect();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let path = to.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|error| file_error("create", &path, error))?;
            continue;
        }
        let mut file =
            fs::File::create(&path).map_err(|error| file_error("create", &path, error))?;
        std::io::copy(&mut entry, &mut file).map_err(|error| file_error("write", &path, error))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .map_err(|error| file_error("set the mode of", &path, error))?;
        }
    }
    Ok(())
}

/// Points the Gradle settings at this engine's plugin and names the root project `name`.
fn set_settings(path: &Path, name: &str) -> Result<()> {
    let text = fs::read_to_string(path).map_err(|error| file_error("read", path, error))?;
    let text = set_quoted(&text, PLUGIN, ENGINE_VERSION)?;
    let text = set_quoted(&text, ROOT_PROJECT, name)?;
    fs::write(path, text).map_err(|error| file_error("write", path, error))
}

/// `text` with the quoted value of the one line that starts with `setting` replaced by `value`.
fn set_quoted(text: &str, setting: &'static str, value: &str) -> Result<String> {
    let mut found = 0;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let indent = &line[..line.len() - line.trim_start().len()];
            if line.trim_start().starts_with(setting) {
                found += 1;
                format!("{indent}{setting}\"{value}\"")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if found != 1 {
        return Err(HostError::TemplateSetting {
            file: SETTINGS,
            setting,
        });
    }
    Ok(lines.join("\n") + "\n")
}
