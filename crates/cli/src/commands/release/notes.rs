// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::process::Command;

use anyhow::{Context, Result, ensure};

pub(super) fn commit_subjects(tag: &str) -> Result<String> {
    let previous = git(&["describe", "--tags", "--abbrev=0", &format!("{tag}^")]).ok();
    let range = previous.map_or_else(
        || tag.to_owned(),
        |previous| format!("{}..{tag}", previous.trim()),
    );
    git(&["log", "--no-merges", "--format=- %s", &range])
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("failed to run git")?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git printed non-UTF-8 output")
}
