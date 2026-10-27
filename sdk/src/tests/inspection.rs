// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use reseam_apk::ScratchDir;
use reseam_model::{
    InspectRequest, InstallMethod, PatchOutput, PatchPreset, PatchRequest, PatchSelection, Problem,
    Trust,
};

use super::apk;
use crate::bundles::trusted_archives;
use crate::inputs::Inputs;
use crate::trust::TrustStore;
use reseam_fixtures::zip;

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
dependencies = [{{ bundle = "other", id = "sample.core" }}]

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
        assert_eq!(patch.spec.reference.id, "sample.patch");
        assert_eq!(
            patch.spec.dependencies,
            [reseam_model::PatchRef::parse("other/sample.core").unwrap()]
        );
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
