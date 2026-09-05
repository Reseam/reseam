// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiling BoltFFI's generated `jni_glue.c` and linking it into the shared
//! library a JVM loads, for the host and for each Android ABI.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::run::run;

/// Only the JNI entry points and BoltFFI's runtime leave the library.
const EXPORTS_MAP: &str = r#"{
    global:
        Java_*;
        JNI_OnLoad*;
        JNI_OnUnload*;
        boltffi_*;
    local:
        *;
};
"#;

pub struct Toolchain {
    cc: PathBuf,
}

impl Toolchain {
    pub fn new(cc: impl Into<PathBuf>) -> Self {
        Self { cc: cc.into() }
    }

    pub fn compile(&self, source: &Path, includes: &[PathBuf], object: &Path) -> Result<()> {
        let mut cmd = Command::new(&self.cc);
        cmd.args(["-c", "-fPIC", "-O2", "-w"]);
        for include in includes {
            cmd.arg("-I").arg(include);
        }
        run(cmd.arg("-o").arg(object).arg(source))
    }

    /// Links `object` and the whole static `archive` into a shared library
    /// that exports only the JNI surface.
    pub fn link(&self, object: &Path, archive: &Path, libs: &[&str], output: &Path) -> Result<()> {
        let exports = object.with_file_name("exports.map");
        std::fs::write(&exports, EXPORTS_MAP)?;
        run(Command::new(&self.cc)
            .args(["-shared", "-o"])
            .arg(output)
            .arg(object)
            .arg("-Wl,--whole-archive")
            .arg(archive)
            .arg("-Wl,--no-whole-archive")
            .args(["-Xlinker", "--version-script", "-Xlinker"])
            .arg(&exports)
            .arg("-Wl,--gc-sections")
            .args(libs))
    }
}
