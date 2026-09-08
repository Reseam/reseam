// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;

use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::resources::{EntryValue, ResEntry, ResPackage, ResType, TypeSpec};
use reseam_apk::{axml, ApkFile, ApplicationIcon, IconLayer, ResValue, ResourceTable, StringPool};

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";
const SPLITWISE_APK: &str = "../../test-apks/com.Splitwise.SplitwiseMobile_26.5.3.apk";

fn strings(values: &[&str]) -> StringPool {
    StringPool::new(values.iter().map(|s| s.to_string()).collect(), true)
}

fn config(language: Option<&[u8; 2]>, density: u16) -> Vec<u8> {
    let mut config = vec![0u8; 48];
    config[0..4].copy_from_slice(&48u32.to_le_bytes());
    if let Some(language) = language {
        config[8..10].copy_from_slice(language);
    }
    config[14..16].copy_from_slice(&density.to_le_bytes());
    config
}

fn entries(id: u8, config: Vec<u8>, entries: &[(usize, ResValue)]) -> ResType {
    let mut res_type = ResType::new(id, config);
    for &(key, value) in entries {
        res_type.set(
            key,
            Some(ResEntry {
                flags: 0,
                key: key as u32,
                value: EntryValue::Simple(value),
            }),
        );
    }
    res_type
}

/// `string/app_name` in the default and French configurations;
/// `mipmap/ic_launcher` as hdpi and xxhdpi bitmaps plus an anydpi adaptive
/// icon; `mipmap/bg` as hdpi and xxhdpi bitmaps; `color/fg`.
fn resources() -> ResourceTable {
    let mut package = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string", "mipmap", "color"]),
        strings(&["app_name", "ic_launcher", "bg", "fg"]),
    );
    package.type_specs.push(TypeSpec::new(1, vec![0]));
    package.type_specs.push(TypeSpec::new(2, vec![0, 0, 0]));
    package.type_specs.push(TypeSpec::new(3, vec![0, 0, 0, 0]));
    let string = ResValue::string;
    package.types.extend([
        entries(1, config(None, 0), &[(0, string(0))]),
        entries(1, config(Some(b"fr"), 0), &[(0, string(1))]),
        entries(2, config(None, 240), &[(1, string(2)), (2, string(5))]),
        entries(2, config(None, 480), &[(1, string(3)), (2, string(6))]),
        entries(2, config(None, 0xFFFE), &[(1, string(4))]),
        entries(
            3,
            config(None, 0),
            &[(3, ResValue::new(ResValue::INT_COLOR_ARGB8, 0xFF11_2233))],
        ),
    ]);
    ResourceTable {
        global_strings: strings(&[
            "Example",
            "Exemple",
            "res/mipmap-hdpi/ic_launcher.png",
            "res/mipmap-xxhdpi/ic_launcher.png",
            "res/mipmap-anydpi-v26/ic_launcher.xml",
            "res/mipmap-hdpi/bg.png",
            "res/mipmap-xxhdpi/bg.png",
        ]),
        packages: vec![package],
    }
}

const MANIFEST: &str = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test">
    <application android:label="@string/app_name" android:icon="@mipmap/ic_launcher" />
</manifest>"#;

const ADAPTIVE_ICON: &str = r#"<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@mipmap/bg" />
    <foreground android:drawable="@color/fg" />
</adaptive-icon>"#;

fn write_apk(path: &Path, manifest: &[u8], entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("AndroidManifest.xml", options).unwrap();
    writer.write_all(manifest).unwrap();
    for (name, data) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn label_prefers_the_default_configuration_and_icon_the_densest_bitmap() {
    let mut table = resources();
    let manifest = axml::compile_xml(MANIFEST, Some(&mut table)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.apk");
    write_apk(
        &path,
        &manifest,
        &[
            ("resources.arsc", &table.serialize().unwrap()),
            ("res/mipmap-hdpi/ic_launcher.png", b"hdpi"),
            ("res/mipmap-xxhdpi/ic_launcher.png", b"xxhdpi"),
            ("res/mipmap-anydpi-v26/ic_launcher.xml", b"<adaptive-icon/>"),
        ],
    );

    let mut apk = ApkFile::open(&path, &ParseOptions::default()).unwrap();
    assert_eq!(apk.application_label().unwrap().as_deref(), Some("Example"));
    assert_eq!(
        apk.application_icon().unwrap(),
        Some(ApplicationIcon::Bitmap(b"xxhdpi".to_vec()))
    );
}

#[test]
fn adaptive_icon_layers_resolve_to_bitmaps_and_colors() {
    let mut table = resources();
    let manifest = axml::compile_xml(MANIFEST, Some(&mut table)).unwrap();
    let adaptive = axml::compile_xml(ADAPTIVE_ICON, Some(&mut table)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.apk");
    write_apk(
        &path,
        &manifest,
        &[
            ("resources.arsc", &table.serialize().unwrap()),
            ("res/mipmap-anydpi-v26/ic_launcher.xml", &adaptive),
            ("res/mipmap-hdpi/bg.png", b"bg-hdpi"),
            ("res/mipmap-xxhdpi/bg.png", b"bg-xxhdpi"),
        ],
    );

    let mut apk = ApkFile::open(&path, &ParseOptions::default()).unwrap();
    assert_eq!(
        apk.application_icon().unwrap(),
        Some(ApplicationIcon::Adaptive {
            background: IconLayer::Bitmap(b"bg-xxhdpi".to_vec()),
            foreground: IconLayer::Color(0xFF11_2233),
        })
    );
}

#[test]
fn literal_label_without_an_icon() {
    let manifest = axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test">
            <application android:label="Literal" />
        </manifest>"#,
        None,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.apk");
    write_apk(&path, &manifest, &[]);

    let mut apk = ApkFile::open(&path, &ParseOptions::default()).unwrap();
    assert_eq!(apk.application_label().unwrap().as_deref(), Some("Literal"));
    assert_eq!(apk.application_icon().unwrap(), None);
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG")
}

#[test]
fn real_apks_resolve_to_their_launcher_name_and_icon() {
    for (path, label) in [
        (YOUTUBE_APK, "YouTube"),
        (INSTAGRAM_APK, "Instagram"),
        (SPLITWISE_APK, "Splitwise"),
    ] {
        if !Path::new(path).exists() {
            continue;
        }
        let mut apk = ApkFile::open(path, &ApkFile::patch_options()).unwrap();
        assert_eq!(apk.application_label().unwrap().as_deref(), Some(label));
        match apk.application_icon().unwrap().unwrap() {
            ApplicationIcon::Bitmap(bytes) => assert!(is_png(&bytes), "{label}"),
            ApplicationIcon::Adaptive {
                background: IconLayer::Bitmap(background),
                foreground: IconLayer::Bitmap(foreground),
            } => assert!(is_png(&background) && is_png(&foreground), "{label}"),
            other => panic!("{label}: {other:?}"),
        }
    }
}
