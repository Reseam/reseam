// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use anyhow::Result;

use crate::{paths, run::run};

/// Generates the SDK's Kotlin and packages its Android and host-desktop JNI
/// libraries. Requires the Android NDK, Rust's Android targets, JAVA_HOME,
/// and the NDK's clang on PATH.
pub fn regen() -> Result<()> {
    run(Command::new("boltffi")
        .args(["pack", "android", "--release", "--deny-skipped"])
        .current_dir(paths::sdk().join("native")))
}
