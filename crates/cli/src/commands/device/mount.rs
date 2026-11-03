// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, bail, ensure};
use reseam_model::{MountRequest, PatchedSplit, Problem};
use reseam_sdk::mount::{
    installed_app, is_mounted, mount_script, query_script, status_script, unmount_script,
};
use reseam_sdk::{ApkInspection, dex_metadata};
use tracing::info;

use super::adb::Adb;
use crate::app::{MountCommand, UnmountCommand};

/// Where the patched files wait on the device until the mount copies them:
/// the shell user can write there, and root can read it.
const STAGING: &str = "/data/local/tmp/reseam-mount";

/// Mounts a build made with `reseam patch --mount` over the app installed on
/// a rooted device.
pub fn run_mount(command: &MountCommand) -> Result<()> {
    let adb = Adb::new(command.device.serial.clone());
    let mut app = ApkInspection::open(&command.apk, &command.split)?;
    let metadata = app.metadata()?;
    let package = metadata
        .identity
        .package_name
        .context("the APK's manifest names no package")?;
    let installed = installed_app(&adb.root(&query_script(&package)?)?)?.ok_or_else(|| {
        Problem::AppNotInstalled {
            package: package.clone(),
        }
    })?;
    let base = format!("{STAGING}/base.apk");
    let mut uploads = vec![(app.base_path().to_owned(), base.clone())];
    let mut splits = Vec::new();
    for (name, path) in app.splits() {
        let device = format!("{STAGING}/split_{name}.apk");
        uploads.push((path.to_owned(), device.clone()));
        splits.push(PatchedSplit {
            name: name.to_owned(),
            path: device,
        });
    }
    // ART reads an installed app's `.dm` first, so the mount covers it when
    // the app has one.
    let dex_metadata = match &installed.dex_metadata_file {
        Some(_) => dex_metadata(app.base_path(), adb.sdk_level()?)?,
        None => None,
    };
    let dex_metadata_file = dex_metadata
        .map(|bytes| {
            let file = tempfile::NamedTempFile::new()?;
            std::fs::write(file.path(), bytes)?;
            anyhow::Ok(file)
        })
        .transpose()?;
    let dex_metadata = dex_metadata_file.as_ref().map(|file| {
        let device = format!("{STAGING}/base.dm");
        uploads.push((file.path().to_owned(), device.clone()));
        device
    });
    let script = mount_script(&MountRequest {
        package_name: package.clone(),
        label: metadata
            .application_label
            .unwrap_or_else(|| package.clone()),
        version_code: metadata
            .identity
            .version_code
            .context("the APK's manifest has no version code")?,
        installed,
        base,
        splits,
        dex_metadata,
    })?;
    adb.shell(&format!("rm -rf {STAGING} && mkdir -p {STAGING}"))?;
    let mounted = uploads
        .iter()
        .try_for_each(|(local, device)| adb.push(local, device))
        .and_then(|()| adb.root(&script));
    adb.shell(&format!("rm -rf {STAGING}"))?;
    mounted?;
    ensure!(
        is_mounted(&adb.root(&status_script(&package)?)?),
        "the root shell ran the mount, but {package} is not mounted"
    );
    info!(%package, "mounted");
    Ok(())
}

/// Takes a mounted build off the installed app.
pub fn run_unmount(command: &UnmountCommand) -> Result<()> {
    let adb = Adb::new(command.device.serial.clone());
    let package = &command.package;
    if !is_mounted(&adb.root(&status_script(package)?)?) {
        bail!("{package} has no mounted build");
    }
    adb.root(&unmount_script(package)?)?;
    info!(%package, "unmounted");
    Ok(())
}
