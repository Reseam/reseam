// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "common/archive.rs"]
mod archive;
use archive::write_apk;

use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::resources::{EntryValue, ResEntry, ResPackage, ResType, TypeSpec};
use reseam_apk::{
    ApkFile, ApplicationIcon, IconLayer, ResValue, ResourceScope, ResourceTable, StringPool, axml,
};

fn strings(values: &[&str]) -> StringPool {
    StringPool::new(
        values.iter().map(ToString::to_string).collect(),
        reseam_apk::StringEncoding::Utf8,
    )
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
        res_type
            .set(
                key,
                Some(ResEntry {
                    flags: 0,
                    key: key as u32,
                    value: EntryValue::Simple(value),
                }),
            )
            .expect("resource fixture entry");
    }
    res_type
}

fn resources() -> ResourceTable {
    let mut package = ResPackage::new(
        0x7F,
        "com.example.test",
        strings(&["string", "mipmap", "color"]),
        strings(&["app_name", "ic_launcher", "bg", "fg"]),
    );
    package.add_type_spec(TypeSpec::new(1, vec![0]));
    package.add_type_spec(TypeSpec::new(2, vec![0, 0, 0]));
    package.add_type_spec(TypeSpec::new(3, vec![0, 0, 0, 0]));
    let string = ResValue::string;
    for res_type in [
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
    ] {
        package.add_type(res_type);
    }
    ResourceTable::new(
        strings(&[
            "Example",
            "Exemple",
            "res/mipmap-hdpi/ic_launcher.png",
            "res/mipmap-xxhdpi/ic_launcher.png",
            "res/mipmap-anydpi-v26/ic_launcher.xml",
            "res/mipmap-hdpi/bg.png",
            "res/mipmap-xxhdpi/bg.png",
        ]),
        vec![package],
    )
}

const MANIFEST: &str = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test">
    <application android:label="@string/app_name" android:icon="@mipmap/ic_launcher" />
</manifest>"#;

const ADAPTIVE_ICON: &str = r#"<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@mipmap/bg" />
    <foreground android:drawable="@color/fg" />
</adaptive-icon>"#;

#[test]
fn application_presentation_resolves_labels_and_bitmap_or_adaptive_icons() {
    struct Case {
        manifest: &'static str,
        files: Vec<(&'static str, Vec<u8>)>,
        label: &'static str,
        icon: Option<ApplicationIcon>,
    }
    let mut table = resources();
    let adaptive = axml::compile_xml(ADAPTIVE_ICON, Some(&mut ResourceScope::from(&mut table)))
        .expect("adaptive fixture");
    for case in [
        Case {
            manifest: MANIFEST,
            files: vec![
                ("res/mipmap-hdpi/ic_launcher.png", b"hdpi".to_vec()),
                ("res/mipmap-xxhdpi/ic_launcher.png", b"xxhdpi".to_vec()),
                (
                    "res/mipmap-anydpi-v26/ic_launcher.xml",
                    b"<adaptive-icon/>".to_vec(),
                ),
            ],
            label: "Example",
            icon: Some(ApplicationIcon::Bitmap(b"xxhdpi".to_vec())),
        },
        Case {
            manifest: MANIFEST,
            files: vec![
                ("res/mipmap-anydpi-v26/ic_launcher.xml", adaptive),
                ("res/mipmap-hdpi/bg.png", b"bg-hdpi".to_vec()),
                ("res/mipmap-xxhdpi/bg.png", b"bg-xxhdpi".to_vec()),
            ],
            label: "Example",
            icon: Some(ApplicationIcon::Adaptive {
                background: IconLayer::Bitmap(b"bg-xxhdpi".to_vec()),
                foreground: IconLayer::Color(0xff11_2233),
            }),
        },
        Case {
            manifest: r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test"><application android:label="Literal"/></manifest>"#,
            files: vec![],
            label: "Literal",
            icon: None,
        },
    ] {
        let manifest = axml::compile_xml(case.manifest, Some(&mut ResourceScope::from(&mut table)))
            .expect("manifest fixture");
        let resources = table.serialize().expect("resources fixture");
        let mut entries: Vec<_> = case
            .files
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect();
        entries.push(("resources.arsc", &resources));
        let dir = tempfile::tempdir().expect("fixture");
        let path = dir.path().join("app.apk");
        write_apk(&path, &manifest, &entries);
        let mut apk = ApkFile::open(&path, ParseOptions::default()).expect("fixture");
        assert_eq!(
            apk.application_label().expect("label").as_deref(),
            Some(case.label)
        );
        assert_eq!(apk.application_icon().expect("icon"), case.icon);
    }
}
