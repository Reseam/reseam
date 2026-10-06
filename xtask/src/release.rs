// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

use semver::Version;
use toml_edit::DocumentMut;

use crate::paths;
use crate::run::run;

/// Sets the workspace version, commits, and tags `v<version>`. A manifest
/// already at that version is only tagged. Requires a clean worktree.
#[expect(
    clippy::print_stdout,
    reason = "The release command reports the created tag"
)]
pub fn release(version: &str) -> Result<()> {
    let version = Version::parse(version)
        .context("version must be valid SemVer")?
        .to_string();
    let root = paths::workspace_root();
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()?;
    ensure!(status.status.success(), "git status failed");
    ensure!(
        status.stdout.is_empty(),
        "commit existing changes before cutting a release"
    );
    let manifest = root.join("Cargo.toml");
    let mut document = fs::read_to_string(&manifest)?.parse::<DocumentMut>()?;
    let current = workspace_version(&document)?;
    if current != version {
        document["workspace"]["package"]["version"] = toml_edit::value(&version);
        let updated = document.to_string();
        fs::write(&manifest, updated)?;
        run(Command::new("cargo")
            .args(["update", "--workspace"])
            .current_dir(&root))?;
        run(Command::new("git")
            .args(["add", "Cargo.toml", "Cargo.lock"])
            .current_dir(&root))?;
        run(Command::new("git")
            .args(["commit", "-m", &format!("chore: release v{version}")])
            .current_dir(&root))?;
    }
    run(Command::new("git")
        .args([
            "tag",
            "-a",
            "-m",
            &format!("v{version}"),
            &format!("v{version}"),
        ])
        .current_dir(&root))?;
    println!("Tagged v{version}; push with `git push --follow-tags`.");
    Ok(())
}

/// Fails unless `tag` is `v<workspace version>`.
pub fn check_tag(tag: &str) -> Result<()> {
    let text = fs::read_to_string(paths::workspace_root().join("Cargo.toml"))?;
    let document = text.parse::<DocumentMut>()?;
    let version = workspace_version(&document)?;
    match tag.strip_prefix('v') {
        Some(tagged) if tagged == version => Ok(()),
        _ => bail!("tag {tag} does not match workspace version {version}"),
    }
}

fn workspace_version(manifest: &DocumentMut) -> Result<&str> {
    manifest
        .get("workspace")
        .and_then(|item| item.get("package"))
        .and_then(|item| item.get("version"))
        .and_then(toml_edit::Item::as_str)
        .context("no version under [workspace.package] in Cargo.toml")
}
