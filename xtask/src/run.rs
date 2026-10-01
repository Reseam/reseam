// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};

pub fn run(cmd: &mut Command) -> Result<()> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {program}"))?;
    ensure!(status.success(), "{program} exited with {status}");
    Ok(())
}

/// Captures both streams and reports launch or exit failures with their diagnostics.
pub fn output(cmd: &mut Command) -> Result<Output> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let output = cmd
        .output()
        .with_context(|| format!("failed to spawn {program}"))?;
    ensure!(
        output.status.success(),
        "{program} exited with {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(output)
}

pub fn gradle(arguments: &[&str]) -> Result<()> {
    let root = crate::paths::workspace_root();
    let launcher = root.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });
    run(Command::new(launcher)
        .args(arguments)
        .args(["--no-daemon", "-Dorg.gradle.jvmargs=-Xmx1536m"])
        .current_dir(root))
}
