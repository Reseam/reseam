// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{Cursor, Write};
use std::path::Path;

use reseam_apk::{scratch::ScratchDir, ApkFile, ContainerFormat};

use crate::inspect::open_apk;
use crate::metrics::PatchProfiler;
use crate::output::write_signed;
use crate::{inspect_apk, patch, PatchArtifact, PatchOutput, PatchRequest};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn apk(split: &str) -> Vec<u8> {
    let manifest = reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" {split}><application android:label="Example" /></manifest>"#
    ), None).unwrap();
    zip(&[("AndroidManifest.xml", &manifest)])
}

#[test]
fn containers_and_plain_apks_use_the_same_output_pipeline() {
    let tmp = ScratchDir::new("sdk-output-test").unwrap();
    let base = apk("");
    let split = apk(r#"split="config.en""#);
    for count in [1, 2] {
        for (extension, metadata) in [
            ("apkm", None),
            (
                "xapk",
                Some(br#"{"xapk_version":2,"package_name":"com.example.test"}"#.as_slice()),
            ),
            (
                "apk",
                Some(br#"{"xapk_version":2,"package_name":"com.example.test"}"#.as_slice()),
            ),
        ] {
            let mut entries = vec![("base.apk", base.as_slice())];
            if count == 2 {
                entries.push(("config.en.apk", split.as_slice()));
            }
            if let Some(metadata) = metadata {
                entries.push(("manifest.json", metadata));
            }
            let input = tmp.path().join(format!("input-{count}.{extension}"));
            std::fs::write(&input, zip(&entries)).unwrap();
            let metadata = inspect_apk(&input, &[]).unwrap();
            assert_eq!(metadata.application_label.as_deref(), Some("Example"));
            assert_eq!(metadata.component_count, count);
            assert_eq!(
                metadata.bundle_kind,
                Some(if extension == "apkm" {
                    ContainerFormat::Apkm
                } else {
                    ContainerFormat::Xapk
                })
            );
            let destination = tmp.path().join(format!("output-{count}-{extension}"));
            let opened = open_apk(&input, &[], &ApkFile::patch_options()).unwrap();
            let output = PatchOutput::Auto {
                path: destination.clone(),
            }
            .resolve(opened.apk.components().len())
            .unwrap();
            let extracted = opened.bundle.as_ref().unwrap().base_path().to_path_buf();
            write_signed(opened.apk, &output, None, &mut PatchProfiler::new()).unwrap();
            drop(opened.bundle);
            assert!(!extracted.exists());
            match output {
                PatchArtifact::SingleFile { path } => {
                    assert_eq!(path, destination.with_extension("apk"));
                    assert_eq!(inspect_apk(&path, &[]).unwrap().component_count, 1);
                }
                PatchArtifact::SplitDir { path } => {
                    assert_eq!(path, destination);
                    let metadata =
                        inspect_apk(&path.join("base.apk"), &[path.join("config.en.apk")]).unwrap();
                    assert_eq!(metadata.component_count, 2);
                }
            }
        }
    }
    // Explicit directory output also works for an ordinary single APK.
    let input = tmp.path().join("plain.apk");
    std::fs::write(&input, base).unwrap();
    let opened = open_apk(&input, &[], &ApkFile::patch_options()).unwrap();
    assert!(opened.bundle.is_none());
    let destination = tmp.path().join("chosen-directory");
    let output = PatchOutput::SplitDir {
        path: destination.clone(),
    }
    .resolve(1)
    .unwrap();
    write_signed(opened.apk, &output, None, &mut PatchProfiler::new()).unwrap();
    assert!(destination.join("plain.apk").is_file());
}

#[test]
fn incompatible_inputs_and_output_fail_before_loading_patches() {
    let tmp = ScratchDir::new("sdk-input-test").unwrap();
    let input = tmp.path().join("app.apkm");
    std::fs::write(
        &input,
        zip(&[
            ("base.apk", &apk("")),
            ("config.en.apk", &apk(r#"split="config.en""#)),
        ]),
    )
    .unwrap();
    assert!(inspect_apk(&input, &[Path::new("extra.apk").into()])
        .unwrap_err()
        .to_string()
        .contains("cannot be combined"));
    let error = patch(
        &PatchRequest {
            apk_path: input,
            split_paths: Vec::new(),
            bundle_paths: Vec::new(),
            trust: Default::default(),
            selection: Default::default(),
            output: PatchOutput::SingleFile {
                path: tmp.path().join("out.apk"),
            },
            signing: None,
            dry_run: true,
        },
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("input has splits"), "{error}");
    assert!(!tmp.path().join("out.apk").exists());
}
