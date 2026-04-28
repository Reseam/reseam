// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Context, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

pub struct Cmd {
    inner: Command,
    label: String,
}

impl Cmd {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        let program = program.as_ref().to_owned();
        let label = program.to_string_lossy().into_owned();
        Self {
            inner: Command::new(program),
            label,
        }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.inner.arg(a);
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub fn env(mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> Self {
        self.inner.env(k, v);
        self
    }

    pub fn cwd(mut self, dir: impl AsRef<Path>) -> Self {
        self.inner.current_dir(dir);
        self
    }

    pub fn run(mut self) -> Result<()> {
        let status = self
            .inner
            .status()
            .with_context(|| format!("failed to spawn {}", self.label))?;
        if !status.success() {
            bail!("{} exited with {}", self.label, status);
        }
        Ok(())
    }
}
