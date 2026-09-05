// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use anyhow::{ensure, Context, Result};

pub fn run(cmd: &mut Command) -> Result<()> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {program}"))?;
    ensure!(status.success(), "{program} exited with {status}");
    Ok(())
}
