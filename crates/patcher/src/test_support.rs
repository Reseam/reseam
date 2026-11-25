// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use reseam_apk::{ApkFile, axml};
use reseam_model::Abi;

/// The `reseam-elf` test library, built from `crates/elf/fixtures` for arm64-v8a.
pub(crate) const NATIVE_LIBRARY: &[u8] = include_bytes!("../../elf/fixtures/library.so");
/// The same library built for armeabi-v7a.
pub(crate) const NATIVE_LIBRARY_ARM: &[u8] = include_bytes!("../../elf/fixtures/library-arm.so");

/// An app with two activities, opened for patching.
pub(crate) fn apk() -> (tempfile::TempDir, ApkFile) {
    apk_with(
        r#"<application><activity android:name=".Main"/><activity android:name=".Other"/></application>"#,
        &[],
        &[],
    )
}

/// `com.example` 2.0 with the `<application>` element `application` and `entries`, opened for
/// patching for a device whose ABIs are `device`.
pub(crate) fn apk_with(
    application: &str,
    entries: &[(&str, &[u8])],
    device: &[Abi],
) -> (tempfile::TempDir, ApkFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.apk");
    let manifest = axml::compile_xml(
        &format!(
            r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example" android:versionName="2.0"><uses-sdk android:minSdkVersion="21"/>{application}</manifest>"#
        ),
        None,
    )
    .unwrap();
    std::fs::write(&path, reseam_fixtures::apk(&manifest, entries)).unwrap();
    let apk = ApkFile::open_split(path, &[] as &[&Path], device, ApkFile::patch_options()).unwrap();
    (dir, apk)
}
