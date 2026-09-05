// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use reseam_apk::reseam_dex::ParseOptions;
use reseam_apk::{ApkFile, ApkWriteOptions};

const YOUTUBE_APK: &str = "../../test-apks/for_testing_com.google.android.youtube_21.10.494.apk";
const INSTAGRAM_APK: &str = "../../test-apks/com.instagram.android_419.0.0.49.71-382508603_minAPI28(arm64-v8a)(360,400,420,480dpi)_apkmirror.com.apk";

fn available_apks() -> Vec<&'static str> {
    [YOUTUBE_APK, INSTAGRAM_APK]
        .into_iter()
        .filter(|p| Path::new(p).exists())
        .collect()
}

fn lazy() -> ParseOptions {
    ParseOptions {
        lazy: true,
        ..ParseOptions::default()
    }
}

fn open_lazy(path: impl AsRef<Path>) -> ApkFile {
    ApkFile::open(path, &lazy()).expect("open failed")
}

fn open_eager(path: impl AsRef<Path>) -> ApkFile {
    ApkFile::open(path, &ParseOptions::default()).expect("open failed")
}

fn write(apk: &mut ApkFile, dir: &Path) -> PathBuf {
    let mut paths = apk
        .write_to(dir, ApkWriteOptions::default())
        .expect("write_to failed");
    paths.remove(0)
}

fn global_string_count(apk: &mut ApkFile) -> usize {
    apk.base_mut()
        .resources()
        .expect("resources")
        .map(|r| r.global_strings.len())
        .unwrap_or(0)
}

fn dex_entry_count(bytes: &[u8]) -> usize {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("not a valid ZIP");
    assert!(
        archive.index_for_name("AndroidManifest.xml").is_some(),
        "missing AndroidManifest.xml"
    );
    archive
        .file_names()
        .filter(|name| name.ends_with(".dex"))
        .count()
}

/// Returns early from the first `count` non-trivial methods in `apk`.
fn patch_methods(apk: &mut ApkFile, count: usize) -> usize {
    let mut patched = 0;
    for i in 0..apk.dex().len() {
        let dex = apk.dex_mut(i).expect("dex access failed");
        dex.resolve_all_class_data().expect("resolve");
        for class in dex.classes.iter_resident_mut() {
            let Some(data) = &mut class.class_data else {
                continue;
            };
            for method in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                if let Some(code) = &mut method.code {
                    if code.instructions.len() > 5 && patched < count {
                        code.return_early();
                        patched += 1;
                    }
                }
            }
            if patched >= count {
                return patched;
            }
        }
    }
    patched
}

#[test]
fn test_apk_full_round_trip() {
    let apks = available_apks();
    if apks.is_empty() {
        return;
    }

    for apk_path in &apks {
        let mut apk = open_lazy(apk_path);
        let original_dex_count = apk.dex().len();
        let original_package = apk.package_name().map(|s| s.into_owned());
        let original_resource_strings = global_string_count(&mut apk);

        let tmp = tempfile::tempdir().expect("tempdir failed");
        let output_path = write(&mut apk, tmp.path());
        drop(apk);
        assert_eq!(
            dex_entry_count(&std::fs::read(&output_path).unwrap()),
            original_dex_count
        );

        let mut apk2 = open_lazy(&output_path);
        assert_eq!(
            apk2.package_name().map(|s| s.into_owned()),
            original_package
        );
        assert_eq!(apk2.dex().len(), original_dex_count);
        assert_eq!(global_string_count(&mut apk2), original_resource_strings);
        drop(apk2);

        let mut apk3 = open_eager(apk_path);
        let patched = patch_methods(&mut apk3, 3);
        assert_eq!(patched, 3);
        let tmp3 = tempfile::tempdir().expect("tempdir failed");
        let out3 = write(&mut apk3, tmp3.path());
        let apk3r = open_eager(&out3);
        assert_eq!(apk3r.dex().len(), original_dex_count);
        assert_eq!(
            apk3r.package_name().map(|s| s.into_owned()),
            original_package
        );
        drop((apk3, apk3r));

        let mut apk4 = open_lazy(apk_path);
        apk4.base_mut().manifest_mut().set_version_code(99999);
        apk4.base_mut()
            .manifest_mut()
            .set_version_name("99.0.0-reseam");
        let tmp4 = tempfile::tempdir().expect("tempdir failed");
        let out4 = write(&mut apk4, tmp4.path());
        let apk4r = open_lazy(&out4);
        assert_eq!(apk4r.version_code(), Some(99999));
        assert_eq!(apk4r.version_name().as_deref(), Some("99.0.0-reseam"));
        drop((apk4, apk4r));

        let mut apk5 = open_lazy(apk_path);
        let tmp5 = tempfile::tempdir().expect("tempdir failed");
        let out5 = write(&mut apk5, tmp5.path());
        drop(apk5);
        let unsigned_bytes = std::fs::read(&out5).expect("read failed");
        let key = reseam_sign::SigningKey::generate().expect("keygen failed");
        let signed_bytes = reseam_sign::v2::sign(&unsigned_bytes, &key).expect("signing failed");
        assert!(signed_bytes.len() > unsigned_bytes.len());
        let dex_count = dex_entry_count(&signed_bytes);
        assert!(dex_count > 0, "no DEX in signed APK");

        let mut apk6 = open_eager(apk_path);
        apk6.base_mut().manifest_mut().set_version_code(12345);
        apk6.base_mut()
            .manifest_mut()
            .set_version_name("1.0.0-test");
        patch_methods(&mut apk6, 2);
        let tmp6 = tempfile::tempdir().expect("tempdir failed");
        let out6 = write(&mut apk6, tmp6.path());
        drop(apk6);
        let unsigned6 = std::fs::read(&out6).expect("read failed");
        let key6 = reseam_sign::SigningKey::generate().expect("keygen failed");
        let signed6 = reseam_sign::v2::sign(&unsigned6, &key6).expect("signing failed");
        let signed_path = tmp6.path().join("signed.apk");
        std::fs::write(&signed_path, &signed6).expect("write signed failed");
        let apk6r = open_eager(&signed_path);
        assert_eq!(
            apk6r.package_name().map(|s| s.into_owned()),
            original_package
        );
        assert_eq!(apk6r.version_code(), Some(12345));
        assert_eq!(apk6r.version_name().as_deref(), Some("1.0.0-test"));
    }
}
