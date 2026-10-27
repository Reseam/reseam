// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use reseam_apk::{ContainerFormat, ScratchDir};
use reseam_model::{PatchArtifact, PatchOutput, Problem, SigningKeyFiles};

use super::{apk, apk_metadata, publish};
use crate::apk::OpenedApk;
use crate::output::resolve_output;
use reseam_fixtures::zip;

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
            let metadata = apk_metadata(&input, &[]).unwrap();
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
            let opened = OpenedApk::open(&input, &[]).unwrap();
            let output = resolve_output(
                &PatchOutput::Auto {
                    path: destination.display().to_string(),
                },
                opened.apk.components().len(),
            )
            .unwrap();
            let extracted = opened.apk.base().path().to_path_buf();
            publish(opened.apk, &output, None).unwrap();
            assert!(!extracted.exists());
            match output {
                PatchArtifact::SingleFile { path } => {
                    let path = Path::new(&path);
                    assert_eq!(apk_metadata(path, &[]).unwrap().component_count, 1);
                }
                PatchArtifact::SplitDir { path } => {
                    let path = Path::new(&path);
                    let metadata =
                        apk_metadata(&path.join("base.apk"), &[path.join("config.en.apk")])
                            .unwrap();
                    assert_eq!(metadata.component_count, 2);
                }
            }
        }
    }
    let input = tmp.path().join("plain.apk");
    std::fs::write(&input, base).unwrap();
    let opened = OpenedApk::open(&input, &[]).unwrap();
    assert!(opened.bundle.is_none());
    let destination = tmp.path().join("chosen-directory");
    let output = resolve_output(
        &PatchOutput::SplitDir {
            path: destination.display().to_string(),
        },
        1,
    )
    .unwrap();
    publish(opened.apk, &output, None).unwrap();
    assert!(destination.join("plain.apk").is_file());
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
        let opened = OpenedApk::open(&base, std::slice::from_ref(&split)).unwrap();
        let output = PatchArtifact::SplitDir {
            path: dir.display().to_string(),
        };
        assert!(publish(opened.apk, &output, None).is_err());
        assert_eq!(std::fs::read(&prior).unwrap(), b"prior output");
        assert!(dir.join(blocked).is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    }
    for extension in ["pk8", "der"] {
        let path = tmp.path().join(format!("identity.{extension}"));
        let opened = OpenedApk::open(&base, &[]).unwrap();
        let output = PatchArtifact::SingleFile {
            path: path.display().to_string(),
        };
        assert!(publish(opened.apk, &output, None).is_err());
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
        let opened = OpenedApk::open(&base, &[]).unwrap();
        let output = PatchArtifact::SingleFile {
            path: destination.display().to_string(),
        };
        let signing = SigningKeyFiles {
            key: alias.display().to_string(),
            cert: cert.display().to_string(),
        };
        assert!(publish(opened.apk, &output, Some(&signing)).is_err());
        assert_eq!(std::fs::read(destination).unwrap(), original_key);
    }
}

#[test]
fn output_selection_honors_component_counts_and_explicit_destinations() {
    let path = "out/com.example.app.reseamed";
    let file = |path: &str| Some(PatchArtifact::SingleFile { path: path.into() });
    let directory = || Some(PatchArtifact::SplitDir { path: path.into() });
    for (kind, expected) in [
        ("auto", [file(&format!("{path}.apk")), directory()]),
        ("split_dir", [directory(), directory()]),
        ("single_file", [file(path), None]),
    ] {
        let output: PatchOutput =
            serde_json::from_value(serde_json::json!({"kind": kind, "path": path})).unwrap();
        for (count, expected) in [1, 2].into_iter().zip(expected) {
            let actual = resolve_output(&output, count);
            if let Some(expected) = expected {
                assert_eq!(actual.unwrap(), expected);
            } else {
                assert!(matches!(
                    actual,
                    Err(crate::HostError::Problem(
                        Problem::SingleFileComponents { .. }
                    ))
                ));
            }
        }
    }
}
