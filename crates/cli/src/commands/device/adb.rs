// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, ensure};
use reseam_model::{MountNamespace, RootScript};

/// A device `adb` reaches: the one named, or the only one connected.
pub(super) struct Adb {
    serial: Option<String>,
}

impl Adb {
    pub(super) fn new(serial: Option<String>) -> Self {
        Self { serial }
    }

    /// Runs `command` in the device's shell and returns what it printed.
    pub(super) fn shell(&self, command: &str) -> Result<String> {
        run(
            self.adb().args(["shell", command]).stdin(Stdio::null()),
            command,
        )
    }

    /// Runs `command` on the device with `input` as its standard input, byte
    /// for byte. `exec-in` returns nothing the command prints.
    pub(super) fn exec_in(&self, command: &str, input: &Path) -> Result<()> {
        let input =
            File::open(input).with_context(|| format!("failed to open {}", input.display()))?;
        run(self.adb().args(["exec-in", command]).stdin(input), command).map(drop)
    }

    /// Runs `script` in a root shell in the mount namespace it asks for.
    pub(super) fn root(&self, script: &RootScript) -> Result<String> {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(script.script.as_bytes())?;
        let shell = match script.namespace {
            MountNamespace::Global => "sh",
            MountNamespace::Private => "unshare -m sh",
        };
        let command = format!("su --mount-master -c '{shell}'");
        run(
            self.adb().args(["shell", &command]).stdin(file.reopen()?),
            &command,
        )
    }

    /// Copies `local` to `remote` on the device.
    pub(super) fn push(&self, local: &Path, remote: &str) -> Result<()> {
        let local_text = local.display().to_string();
        run(self.adb().args(["push", &local_text, remote]), "push").map(drop)
    }

    /// The device's Android API level.
    pub(super) fn sdk_level(&self) -> Result<u32> {
        let level = self.shell("getprop ro.build.version.sdk")?;
        level
            .trim()
            .parse()
            .with_context(|| format!("the device reports API level {level:?}"))
    }

    fn adb(&self) -> Command {
        let mut command = Command::new("adb");
        if let Some(serial) = &self.serial {
            command.args(["-s", serial]);
        }
        command
    }
}

fn run(command: &mut Command, what: &str) -> Result<String> {
    let Output {
        status,
        stdout,
        stderr,
    } = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to run adb; install Android platform-tools and put adb on PATH")?;
    ensure!(
        status.success(),
        "adb {what} failed: {}",
        String::from_utf8_lossy(&stderr).trim()
    );
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}
