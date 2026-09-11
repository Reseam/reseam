// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use anyhow::{ensure, Context, Result};

/// Check before either generator writes output, including for `regen all`.
pub fn check_version() -> Result<()> {
    let version = include_str!("../../.boltffi-version").trim();
    let install = format!("cargo install boltffi_cli --version '={version}' --locked");
    let output = Command::new("boltffi")
        .arg("--version")
        .output()
        .with_context(|| {
            format!("failed to run boltffi; install the pinned generator with `{install}`")
        })?;
    ensure!(
        output.status.success(),
        "boltffi --version failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let actual = String::from_utf8_lossy(&output.stdout);
    ensure!(
        actual.trim() == format!("boltffi {version}"),
        "expected boltffi {version}, found {:?}; install the pinned generator with `{install}`",
        actual.trim()
    );
    Ok(())
}
