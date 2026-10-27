// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ApkFile;
use reseam_dex::ParseOptions;

pub fn entry(apk: &mut ApkFile, component: usize, name: &str) -> Option<Vec<u8>> {
    apk.read_component_entry(component, name).unwrap()
}

pub fn manifest_with_application(class: &str) -> Vec<u8> {
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="1.0-base"><application android:name="{class}" /></manifest>"#
    ), None).unwrap()
}

pub fn manifest_bytes(version_name: &str, split_name: Option<&str>) -> Vec<u8> {
    let split_attr = split_name
        .map(|name| format!(r#" split="{name}""#))
        .unwrap_or_default();
    reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="{version_name}"{split_attr} />"#
    ), None).unwrap()
}

pub const LAYOUT_PATH: &str = "res/Qy.xml";

pub fn open_dex_apk(dex_files: &[&[u8]]) -> (tempfile::TempDir, ApkFile) {
    open_split_test_apk_with(&manifest_bytes("1.0-base", None), dex_files)
}

pub fn open_split_test_apk() -> (tempfile::TempDir, ApkFile) {
    open_dex_apk(&[])
}

/** An app with an `Application`, which a settings host needs to start the app runtime. */
pub fn open_settings_test_apk() -> (tempfile::TempDir, ApkFile) {
    open_split_test_apk_with(
        &manifest_with_application("com.example.App"),
        &[dex!("entry_app"), dex!("entry_base")],
    )
}

pub const SETTINGS_SCHEMA: &str = "assets/reseam/runtime-test-bundle/settings.json";

pub fn open_split_test_apk_with(
    base_manifest: &[u8],
    dex_files: &[&[u8]],
) -> (tempfile::TempDir, ApkFile) {
    let tmp = tempfile::tempdir().unwrap();
    let base_path = tmp.path().join("base.apk");
    let split_path = tmp.path().join("config.apk");
    let layout = include_bytes!("../../fixtures/layout.xml").as_slice();
    let base_resources = include_bytes!("../../fixtures/base.arsc").as_slice();
    let split_resources = include_bytes!("../../fixtures/split.arsc").as_slice();

    let dex_bytes: Vec<(String, &[u8])> = dex_files
        .iter()
        .enumerate()
        .map(|(index, dex)| {
            let name = match index {
                0 => "classes.dex".to_string(),
                n => format!("classes{}.dex", n + 1),
            };
            (name, *dex)
        })
        .collect();
    let mut base_entries: Vec<(&str, &[u8])> =
        vec![("resources.arsc", base_resources), (LAYOUT_PATH, layout)];
    base_entries.extend(
        dex_bytes
            .iter()
            .map(|(name, bytes)| (name.as_str(), *bytes)),
    );

    std::fs::write(
        &base_path,
        reseam_fixtures::apk(base_manifest, &base_entries),
    )
    .unwrap();
    std::fs::write(
        &split_path,
        reseam_fixtures::apk(
            &manifest_bytes("1.0-split", Some("config.test")),
            &[("resources.arsc", split_resources)],
        ),
    )
    .unwrap();

    let apk = ApkFile::open_split(
        &base_path,
        &[split_path.as_path()],
        ParseOptions {
            classes: reseam_dex::Loading::Deferred,
            ..ParseOptions::default()
        },
    )
    .unwrap();

    (tmp, apk)
}
