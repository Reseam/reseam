// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::{ApkFile, axml};

/// An app with two activities, opened for patching.
pub(crate) fn apk() -> (tempfile::TempDir, ApkFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.apk");
    let manifest = axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example" android:versionName="2.0"><uses-sdk android:minSdkVersion="21"/><application><activity android:name=".Main"/><activity android:name=".Other"/></application></manifest>"#,
        None,
    )
    .unwrap();
    std::fs::write(&path, reseam_fixtures::apk(&manifest, &[])).unwrap();
    let apk = ApkFile::open(path, ApkFile::patch_options()).unwrap();
    (dir, apk)
}
