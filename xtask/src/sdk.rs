// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use anyhow::Result;

use crate::{paths, run::run};

/// Generates Kotlin and packages Android plus current-host desktop JNI libraries.
///
/// BoltFFI owns compilation, symbol visibility, linking, and library filenames.
/// Requires the Android NDK, installed Rust targets, a JDK in JAVA_HOME, and host clang on PATH.
pub fn regen() -> Result<()> {
    run(Command::new("boltffi")
        .args(["pack", "android", "--release", "--deny-skipped"])
        .current_dir(paths::sdk().join("native")))
}
