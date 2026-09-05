// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cutting a release: the workspace version is the only version there is,
//! and the tag must name it.

use std::fs;
use std::process::Command;

use anyhow::{bail, ensure, Context, Result};

use crate::paths;
use crate::run::run;

/// Sets the workspace version, commits, and tags `v<version>`. A manifest
/// already at that version is only tagged.
pub fn release(version: &str) -> Result<()> {
    ensure!(
        is_semver(version),
        "version must be MAJOR.MINOR.PATCH, got `{version}`"
    );
    let root = paths::workspace_root();
    let manifest = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)?;
    let current = workspace_version(&text)?;
    if current != version {
        let updated = text.replacen(
            &format!("version = \"{current}\""),
            &format!("version = \"{version}\""),
            1,
        );
        fs::write(&manifest, updated)?;
        run(Command::new("cargo")
            .args(["update", "--workspace"])
            .current_dir(&root))?;
        run(Command::new("git")
            .args(["commit", "-am", &format!("chore: release v{version}")])
            .current_dir(&root))?;
    }
    run(Command::new("git")
        .args(["tag", &format!("v{version}")])
        .current_dir(&root))?;
    println!("Tagged v{version}; push with `git push --follow-tags`.");
    Ok(())
}

/// Fails unless `tag` is `v<workspace version>`.
pub fn check_tag(tag: &str) -> Result<()> {
    let text = fs::read_to_string(paths::workspace_root().join("Cargo.toml"))?;
    let version = workspace_version(&text)?;
    match tag.strip_prefix('v') {
        Some(tagged) if tagged == version => Ok(()),
        _ => bail!("tag {tag} does not match workspace version {version}"),
    }
}

fn workspace_version(manifest: &str) -> Result<String> {
    manifest
        .split("[workspace.package]")
        .nth(1)
        .and_then(|section| {
            section
                .lines()
                .find_map(|line| line.trim().strip_prefix("version = "))
        })
        .map(|value| value.trim_matches('"').to_string())
        .context("no version under [workspace.package] in Cargo.toml")
}

fn is_semver(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}
