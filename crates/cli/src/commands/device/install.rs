// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use reseam_sdk::{ApkInspection, dex_metadata};
use tracing::info;

use super::adb::Adb;
use crate::app::InstallCommand;

/// Installs an app over adb with the dex metadata of each APK, so the device
/// compiles it with its baseline profile at install.
pub fn run_install(command: &InstallCommand) -> Result<()> {
    let adb = Adb::new(command.device.serial.clone());
    let sdk = adb.sdk_level()?;
    let app = ApkInspection::open(&command.apk, &command.split)?;
    let scratch = tempfile::tempdir()?;
    // The installer pairs each `.dm` with the APK of the same name; numbering
    // the files keeps names out of the shell.
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for (index, apk) in std::iter::once(app.base_path())
        .chain(app.splits().map(|(_, path)| path))
        .enumerate()
    {
        files.push((format!("{index}.apk"), apk.to_owned()));
        if let Some(metadata) = dex_metadata(apk, sdk)? {
            let path = scratch.path().join(format!("{index}.dm"));
            std::fs::write(&path, metadata)?;
            files.push((format!("{index}.dm"), path));
        }
    }
    let created = adb.shell("cmd package install-create -r")?;
    let session = created
        .split(['[', ']'])
        .nth(1)
        .with_context(|| format!("the device opened no install session: {}", created.trim()))?
        .to_owned();
    if let Err(error) = write_and_commit(&adb, &session, &files) {
        if let Err(abandon) = adb.shell(&format!("cmd package install-abandon {session}")) {
            return Err(error.context(format!(
                "abandoning install session {session} failed too: {abandon}"
            )));
        }
        return Err(error);
    }
    info!(apks = files.len(), "installed");
    Ok(())
}

fn write_and_commit(adb: &Adb, session: &str, files: &[(String, PathBuf)]) -> Result<()> {
    for (name, file) in files {
        let size = file
            .metadata()
            .with_context(|| format!("failed to read {}", file.display()))?
            .len();
        adb.exec_in(
            &format!("cmd package install-write -S {size} {session} {name} -"),
            file,
        )?;
    }
    let result = adb.shell(&format!("cmd package install-commit {session}"))?;
    if !result.contains("Success") {
        bail!("the device refused the install: {}", result.trim());
    }
    Ok(())
}
