// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{Cursor, Write};
use std::path::Path;

use reseam_apk::{ApkFile, ContainerFormat, ScratchDir};

use crate::error::Problem;
use crate::inspect::open_apk;
use crate::metrics::PatchProfiler;
use crate::output::write_signed;
use crate::{PatchArtifact, PatchOutput, PatchRequest, inspect_apk, patch};

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
fn an_unreadable_resource_table_is_an_unreadable_apk() {
    let tmp = ScratchDir::new("sdk-unreadable-resources").unwrap();
    let manifest = reseam_apk::axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1"><application android:label="@0x7f010000" /></manifest>"#,
        None,
    )
    .unwrap();
    let broken = zip(&[
        ("AndroidManifest.xml", &manifest),
        (
            "resources.arsc",
            &[0x02, 0x00, 0x0c, 0x00, 0xff, 0xff, 0x00, 0x00],
        ),
    ]);
    for (name, bytes) in [
        ("broken.apk", broken.clone()),
        ("broken.apkm", zip(&[("base.apk", &broken)])),
    ] {
        let input = tmp.path().join(name);
        std::fs::write(&input, bytes).unwrap();
        let error = inspect_apk(&input, &[]).unwrap_err();
        assert_eq!(
            crate::error::classify(&error),
            Problem::unreadable_apk(&input),
            "{error:#}"
        );
    }
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
            let opened = open_apk(&input, &[], ApkFile::patch_options()).unwrap();
            let output = PatchOutput::Auto {
                path: destination.display().to_string(),
            }
            .resolve(opened.apk.components().len())
            .unwrap();
            let extracted = opened.apk.base().path().to_path_buf();
            write_signed(opened.apk, &output, None, &mut PatchProfiler::new()).unwrap();
            assert!(!extracted.exists());
            match output {
                PatchArtifact::SingleFile { path } => {
                    let path = Path::new(&path);
                    assert_eq!(inspect_apk(path, &[]).unwrap().component_count, 1);
                }
                PatchArtifact::SplitDir { path } => {
                    let path = Path::new(&path);
                    let metadata =
                        inspect_apk(&path.join("base.apk"), &[path.join("config.en.apk")]).unwrap();
                    assert_eq!(metadata.component_count, 2);
                }
            }
        }
    }
    let input = tmp.path().join("plain.apk");
    std::fs::write(&input, base).unwrap();
    let opened = open_apk(&input, &[], ApkFile::patch_options()).unwrap();
    assert!(opened.bundle.is_none());
    let destination = tmp.path().join("chosen-directory");
    let output = PatchOutput::SplitDir {
        path: destination.display().to_string(),
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
    let extra = tmp.path().join("extra.apk");
    std::fs::write(&extra, apk(r#"split="config.fr""#)).unwrap();
    assert!(inspect_apk(&input, &[extra]).is_err());
    let error = patch(
        &PatchRequest {
            apk_path: input.display().to_string(),
            split_paths: Vec::new(),
            bundle_paths: Vec::new(),
            trust: reseam_model::Trust::default(),
            selection: crate::PatchSelection::default(),
            output: PatchOutput::SingleFile {
                path: tmp.path().join("out.apk").display().to_string(),
            },
            signing: None,
            dry_run: true,
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        crate::error::classify(&error),
        Problem::SingleFileComponents { components: 2 },
        "{error:#}"
    );
    assert!(!tmp.path().join("out.apk").exists());
}

#[test]
fn host_failures_retain_problem_categories_and_sources() {
    use crate::{HostError, sdk_error};
    use reseam_patcher::error::PatcherError;
    let older = || PatcherError::BundleTooOld {
        bundle: "example".into(),
        built: "0.3.0".into(),
        running: "0.15.0".into(),
    };
    let newer = || PatcherError::EngineTooOld {
        bundle: "example".into(),
        built: "1.0.0".into(),
        running: "0.15.0".into(),
    };
    for (error, expected) in [
        (
            HostError::Bundle {
                path: "patches.reseam".into(),
                source: older(),
            },
            Problem::BundleTooOld {
                bundle: "example".into(),
                built: "0.3.0".into(),
                running: "0.15.0".into(),
            },
        ),
        (
            HostError::Patcher(newer()),
            Problem::EngineTooOld {
                bundle: "example".into(),
                built: "1.0.0".into(),
                running: "0.15.0".into(),
            },
        ),
        (
            HostError::Problem(Problem::SingleFileComponents { components: 2 }),
            Problem::SingleFileComponents { components: 2 },
        ),
        (
            HostError::InvalidRequest("invalid host request"),
            Problem::Other,
        ),
    ] {
        let failure = sdk_error(&error);
        assert_eq!(failure.problem, expected);
    }
}

#[test]
fn invalid_artifact_targets_preserve_outputs_and_signing_identity() {
    let tmp = ScratchDir::new("sdk-publication").unwrap();
    let base = tmp.path().join("base.apk");
    let split = tmp.path().join("split.apk");
    std::fs::write(&base, apk("")).unwrap();
    std::fs::write(&split, apk(r#"split="config.en""#)).unwrap();
    for blocked in ["base.apk", "split.apk"] {
        let dir = tmp.path().join(blocked.trim_end_matches(".apk"));
        std::fs::create_dir(&dir).unwrap();
        let prior = dir.join(if blocked == "base.apk" {
            "split.apk"
        } else {
            "base.apk"
        });
        std::fs::write(&prior, b"prior output").unwrap();
        std::fs::create_dir(dir.join(blocked)).unwrap();
        let opened = open_apk(
            &base,
            std::slice::from_ref(&split),
            ApkFile::patch_options(),
        )
        .unwrap();
        let output = PatchArtifact::SplitDir {
            path: dir.display().to_string(),
        };
        assert!(write_signed(opened.apk, &output, None, &mut PatchProfiler::new()).is_err());
        assert_eq!(std::fs::read(&prior).unwrap(), b"prior output");
        assert!(dir.join(blocked).is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    }
    for extension in ["pk8", "der"] {
        let path = tmp.path().join(format!("identity.{extension}"));
        let opened = open_apk(&base, &[], ApkFile::patch_options()).unwrap();
        let output = PatchArtifact::SingleFile {
            path: path.display().to_string(),
        };
        assert!(write_signed(opened.apk, &output, None, &mut PatchProfiler::new()).is_err());
        assert!(!path.exists());
    }
    #[cfg(unix)]
    {
        let destination = tmp.path().join("existing.apk");
        let cert = tmp.path().join("existing.der");
        reseam_sign::SigningKey::load_or_generate(&destination, &cert).unwrap();
        let original_key = std::fs::read(&destination).unwrap();
        let alias = tmp.path().join("key-alias.pk8");
        std::os::unix::fs::symlink(&destination, &alias).unwrap();
        let opened = open_apk(&base, &[], ApkFile::patch_options()).unwrap();
        let output = PatchArtifact::SingleFile {
            path: destination.display().to_string(),
        };
        let signing = crate::SigningKeyFiles {
            key: alias.display().to_string(),
            cert: cert.display().to_string(),
        };
        assert!(
            write_signed(
                opened.apk,
                &output,
                Some(&signing),
                &mut PatchProfiler::new()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(destination).unwrap(), original_key);
    }
}
