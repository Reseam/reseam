// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Getting a patched app onto a device: install files, and mounts on rooted devices.

use boltffi::export;
use reseam_model::{Abi, InstalledApp, MountRequest, RootScript, SdkError};

/// The ABI `name` stands for, as a device's `Build.SUPPORTED_ABIS` lists it.
/// Null for a name Android doesn't use.
#[export]
pub fn abi_named(name: String) -> Option<Abi> {
    name.parse().ok()
}

/// The name Android gives `abi`, such as `arm64-v8a`.
#[export]
pub fn abi_name(abi: Abi) -> String {
    abi.as_str().to_owned()
}

/// Dex metadata to write into an install session beside the base APK at
/// `apkPath`, named like it with `.dm` in place of `.apk`, for a device running
/// API level `sdk`. The installer then compiles the app with its baseline
/// profile. Null when the APK ships no profile or the release reads none at
/// install.
#[export]
pub fn dex_metadata(apk_path: String, sdk: u32) -> Result<Option<Vec<u8>>, SdkError> {
    reseam_sdk::dex_metadata(std::path::Path::new(&apk_path), sdk).map_err(SdkError::from)
}

/// Packs the split APKs at `apkPaths` into a new APKS file at `destination`,
/// each under its file name. Fails when `destination` exists.
#[export]
pub fn write_split_archive(apk_paths: Vec<String>, destination: String) -> Result<(), SdkError> {
    let apks: Vec<std::path::PathBuf> = apk_paths.into_iter().map(Into::into).collect();
    reseam_sdk::write_split_archive(&apks, std::path::Path::new(&destination))
        .map_err(SdkError::from)
}

/// A root script that prints what `installedApp` reads about `packageName`.
#[export]
pub fn mount_query_script(package_name: String) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::query_script(&package_name).map_err(SdkError::from)
}

/// The installed app the output of `mountQueryScript` describes; null when it
/// is not installed.
#[export]
pub fn installed_app(output: String) -> Result<Option<InstalledApp>, SdkError> {
    reseam_sdk::mount::installed_app(&output).map_err(SdkError::from)
}

/// A root script that mounts a patched build over the installed app and keeps
/// it mounted across reboots. Fails with `InstalledAppDiffers` when the build
/// does not match the installed version and splits.
#[export]
pub fn mount_script(request: MountRequest) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::mount_script(&request).map_err(SdkError::from)
}

/// Whether the output of `mountStatusScript` says the build is mounted.
#[export]
pub fn is_mounted(output: String) -> bool {
    reseam_sdk::mount::is_mounted(&output)
}

/// A root script that takes a mounted build off the installed app.
#[export]
pub fn unmount_script(package_name: String) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::unmount_script(&package_name).map_err(SdkError::from)
}

/// A root script that prints whether the patched build covers the installed
/// app, which `isMounted` reads.
#[export]
pub fn mount_status_script(package_name: String) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::status_script(&package_name).map_err(SdkError::from)
}

/// A root script that prints the installed app's own `file` from under its
/// mount.
#[export]
pub fn original_apk_script(package_name: String, file: String) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::original_script(&package_name, &file).map_err(SdkError::from)
}

/// A root script that installs the APK and `.dm` files at `files` as an
/// update of `packageName` and prints the package manager's answer.
#[export]
pub fn install_script(package_name: String, files: Vec<String>) -> Result<RootScript, SdkError> {
    reseam_sdk::mount::install_script(&package_name, &files).map_err(SdkError::from)
}
