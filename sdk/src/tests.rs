// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Cursor, Write};
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use reseam_apk::{ApkFile, ContainerFormat, ScratchDir};

use crate::inspect::{Inputs, open_apk, trusted_archives};
use crate::metrics::{PatchProfiler, measure};
use crate::output::{resolve_output, stage_unsigned};
use crate::trust::TrustStore;
use crate::{ApkInspection, patch};
use reseam_model::Problem;
use reseam_model::{
    ApkMetadata, InspectRequest, InstallMethod, PatchArtifact, PatchOutput, PatchPhase,
    PatchPreset, PatchRequest, PatchSelection, SigningKeyFiles, Trust,
};

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

fn apk_metadata(path: &Path, splits: &[std::path::PathBuf]) -> crate::error::Result<ApkMetadata> {
    ApkInspection::open(path, splits)?.metadata()
}

/// Writes `apk` to `output` and signs it, as a run does after patching.
fn publish(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningKeyFiles>,
) -> crate::error::Result<()> {
    let ((), opening) = measure(PatchPhase::OpenApk, || Ok::<_, crate::HostError>(()))?;
    let mut profiler = PatchProfiler::new(opening);
    stage_unsigned(apk, output, signing, &mut profiler)?.sign_and_publish(&mut profiler)
}

fn catalog_manifest() -> String {
    format!(
        r#"
[bundle]
name = "example"
format_version = 1
engine = "{}"

[files]
"patches.jar" = "{}"

[[patches]]
bundle = "example"
id = "sample.patch"
name = "Example patch"
hidden = false
description = "Static metadata"
enabled_by_default = true
dependencies = ["other/sample.core"]

[patches.compatibility]
kind = "packages"
packages = [{{ package = "com.example.test", versions = ["1.0"] }}]

[[patches.options]]
key = "mode"
title = "Mode"
description = "Choose a mode"
option_type = "string"
required = false
valid_values = ["fast", "slow"]
default_value = {{ type = "string", value = "fast" }}
"#,
        reseam_patcher::bundle::ENGINE_VERSION,
        "00".repeat(32),
    )
}

#[test]
fn inspection_reads_signed_catalogs_without_loading_even_trusted_code() {
    let tmp = ScratchDir::new("sdk-catalog").unwrap();
    let key = SigningKey::from_bytes(&[42; 32]);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    let manifest = catalog_manifest();
    let signature = key.sign(manifest.as_bytes()).to_bytes();
    let path = tmp.path().join("example.reseam");
    let write_bundle = |metadata: &[u8]| {
        std::fs::write(
            &path,
            zip(&[
                (
                    "mimetype",
                    reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes(),
                ),
                ("manifest.toml", metadata),
                ("manifest.pubkey", &key.verifying_key().to_bytes()),
                ("manifest.sig", &signature),
                // This payload cannot be loaded, regardless of the client's trust.
                ("patches.jar", b"not executable code"),
            ]),
        )
        .unwrap();
    };
    write_bundle(manifest.as_bytes());
    let input = tmp.path().join("app.apk");
    std::fs::write(&input, apk("")).unwrap();
    let inspection_request = InspectRequest {
        apk_path: Some(input.display().to_string()),
        split_paths: Vec::new(),
        bundle_paths: vec![path.display().to_string()],
        trust: Trust {
            keys: vec![public_key.clone()],
        },
    };
    check_prepared_trust(&inspection_request, &tmp.path().join("prepared.apk"));
    for trusted in [false, true] {
        let trust = Trust {
            keys: if trusted {
                vec![public_key.clone()]
            } else {
                Vec::new()
            },
        };
        let response = crate::inspect(&InspectRequest {
            apk_path: Some(input.display().to_string()),
            split_paths: Vec::new(),
            bundle_paths: vec![path.display().to_string()],
            trust: trust.clone(),
        })
        .unwrap();
        assert_eq!(response.bundles[0].trusted, trusted);
        assert!(response.bundles[0].problem.is_none());
        assert_eq!(response.patches.len(), 1);
        let patch = &response.patches[0];
        assert_eq!(patch.spec.id, "sample.patch");
        assert_eq!(patch.spec.dependencies, ["other/sample.core"]);
        assert_eq!(
            patch.spec.options[0]
                .default_value
                .as_ref()
                .unwrap()
                .as_str(),
            Some("fast")
        );
        assert_eq!(patch.presets, [PatchPreset::Recommended, PatchPreset::All]);
        assert!(patch.incompatibility.is_some());
        let opened = trusted_archives(
            Inputs::open(None, &[], &[path.display().to_string()])
                .unwrap()
                .bundles,
            &TrustStore::new(&trust).unwrap(),
        );
        if trusted {
            assert!(opened.is_ok());
        } else {
            assert!(matches!(
                crate::error::classify(&opened.err().unwrap()),
                Problem::UntrustedBundle { .. }
            ));
        }
    }
    write_bundle(
        manifest
            .replace("Static metadata", "Tampered metadata")
            .as_bytes(),
    );
    let response = crate::inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![path.display().to_string()],
        trust: Trust::default(),
    })
    .unwrap();
    assert!(response.bundles[0].problem.is_some());
    assert!(response.patches.is_empty());
}

fn check_prepared_trust(inspection_request: &InspectRequest, output: &Path) {
    for approved in [false, true] {
        let prepared = crate::PreparedInspection::open(inspection_request).unwrap();
        let error = prepared
            .patch(
                &PatchRequest {
                    apk_path: inspection_request.apk_path.clone().unwrap(),
                    split_paths: Vec::new(),
                    bundle_paths: inspection_request.bundle_paths.clone(),
                    trust: Trust {
                        keys: if approved {
                            inspection_request.trust.keys.clone()
                        } else {
                            Vec::new()
                        },
                    },
                    selection: PatchSelection::default(),
                    output: PatchOutput::SingleFile {
                        path: output.display().to_string(),
                    },
                    signing: None,
                    dry_run: false,
                    install_method: InstallMethod::Install,
                    leave_out_failed: false,
                },
                |_| {},
            )
            .unwrap_err();
        if approved {
            assert!(
                matches!(
                    error,
                    crate::HostError::Bundle {
                        source: reseam_patcher::error::PatcherError::Bundle(_),
                        ..
                    }
                ),
                "{error}"
            );
        } else {
            assert!(matches!(
                crate::error::classify(&error),
                Problem::UntrustedBundle { .. }
            ));
        }
        assert!(!output.exists());
    }
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
        let error = apk_metadata(&input, &[]).unwrap_err();
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
            let opened = open_apk(&input, &[], ApkFile::patch_options()).unwrap();
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
    let opened = open_apk(&input, &[], ApkFile::patch_options()).unwrap();
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
    assert!(apk_metadata(&input, &[extra]).is_err());
    let error = patch(
        &PatchRequest {
            apk_path: input.display().to_string(),
            split_paths: Vec::new(),
            bundle_paths: Vec::new(),
            trust: Trust::default(),
            selection: PatchSelection::default(),
            output: PatchOutput::SingleFile {
                path: tmp.path().join("out.apk").display().to_string(),
            },
            signing: None,
            dry_run: true,
            install_method: InstallMethod::Install,
            leave_out_failed: false,
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
    use crate::HostError;
    use reseam_model::SdkError;
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
        let failure = SdkError::from(error);
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
        assert!(publish(opened.apk, &output, None).is_err());
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
        let opened = open_apk(&base, &[], ApkFile::patch_options()).unwrap();
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
