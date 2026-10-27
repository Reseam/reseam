// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::tables::{config, package, res_type, simple, strings};
use reseam_apk::{
    ApkFile, ApplicationIcon, IconLayer, ResValue, ResourceScope, ResourceTable, axml,
};
use reseam_dex::ParseOptions;

fn resources() -> ResourceTable {
    let string = |key, index| Some(simple(key, ResValue::STRING, index));
    let types = [
        res_type(1, config(None, 0), [string(0, 0)]),
        res_type(1, config(Some(b"fr"), 0), [string(0, 1)]),
        res_type(2, config(None, 240), [None, string(1, 2), string(2, 5)]),
        res_type(2, config(None, 480), [None, string(1, 3), string(2, 6)]),
        res_type(2, config(None, 0xFFFE), [None, string(1, 4)]),
        res_type(
            3,
            config(None, 0),
            [
                None,
                None,
                None,
                Some(simple(3, ResValue::INT_COLOR_ARGB8, 0xFF11_2233)),
            ],
        ),
    ];
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
        vec![package(
            "com.example.test",
            &["string", "mipmap", "color"],
            &["app_name", "ic_launcher", "bg", "fg"],
            types,
        )],
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
        std::fs::write(&path, reseam_fixtures::apk(&manifest, &entries)).expect("fixture APK");
        let mut apk = ApkFile::open(&path, ParseOptions::default()).expect("fixture");
        assert_eq!(
            apk.application_label().expect("label").as_deref(),
            Some(case.label)
        );
        assert_eq!(apk.application_icon().expect("icon"), case.icon);
    }
}
