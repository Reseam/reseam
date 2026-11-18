// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::{paths, run::run};

/// Removes this workspace's crates from the build cache and keeps their dependencies. Every
/// release changes the workspace version, so the next one reuses only the dependencies.
pub fn prune() -> Result<()> {
    prune_target(&paths::target_dir())
}

fn prune_target(root: &Path) -> Result<()> {
    for directory in subdirectories(root)? {
        // BoltFFI builds Binding IR in a target directory inside this one.
        if directory.join(".rustc_info.json").is_file() {
            prune_target(&directory)?;
        } else if is_profile(&directory) {
            clean(root, &directory, None)?;
        } else {
            for profile in subdirectories(&directory)? {
                if is_profile(&profile) {
                    clean(root, &profile, Some(&directory))?;
                }
            }
        }
    }
    Ok(())
}

fn clean(root: &Path, profile: &Path, triple: Option<&Path>) -> Result<()> {
    // Cargo builds the `dev` profile into `debug` and every other profile into a directory of its name.
    let profile = match profile.file_name() {
        Some(name) if name == "debug" => "dev".as_ref(),
        name => name.unwrap_or_default(),
    };
    let mut clean = Command::new("cargo");
    clean
        .args(["clean", "--workspace", "--profile"])
        .arg(profile)
        .arg("--target-dir")
        .arg(root)
        .current_dir(paths::workspace_root());
    if let Some(triple) = triple.and_then(Path::file_name) {
        clean.arg("--target").arg(triple);
    }
    run(&mut clean)
}

fn subdirectories(parent: &Path) -> Result<Vec<PathBuf>> {
    let mut directories = Vec::new();
    for entry in fs::read_dir(parent)? {
        let path = entry?.path();
        if path.is_dir() {
            directories.push(path);
        }
    }
    Ok(directories)
}

fn is_profile(directory: &Path) -> bool {
    directory.join(".fingerprint").is_dir()
}
