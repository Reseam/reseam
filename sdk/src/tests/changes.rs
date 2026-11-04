// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use ed25519_dalek::SigningKey;
use reseam_apk::ScratchDir;
use reseam_model::{
    AppIdentity, BuildRecord, ChangesRequest, InspectRequest, InstallMethod, PatchChange,
    PatchMetadata, PatchOutput, PatchRequest, PatchSelection, RecordedPatch, Trust,
};

use super::{apk, catalog_manifest, metadata, reference, spec, write_catalog};
use crate::changes;

fn patch(id: &str, fingerprint: &str) -> PatchMetadata {
    metadata(spec(id, Some(&[])), fingerprint)
}

#[test]
fn changes_compare_a_build_with_the_patches_available_now() {
    let record = BuildRecord {
        app: AppIdentity {
            package_name: Some("com.example".to_owned()),
            version_name: Some("2.0".to_owned()),
            version_code: None,
        },
        selection: PatchSelection::default(),
        install_method: InstallMethod::Install,
        patches: ["kept", "fixed", "removed"]
            .into_iter()
            .map(|id| RecordedPatch {
                patch: reference(id),
                fingerprint: "1".to_owned(),
            })
            .collect(),
    };
    let unchanged = [
        patch("kept", "1"),
        patch("fixed", "1"),
        patch("removed", "1"),
    ];
    for (patches, expected) in [
        (unchanged.to_vec(), vec![]),
        (
            vec![patch("kept", "1"), patch("fixed", "2"), patch("added", "1")],
            vec![
                PatchChange::Changed {
                    patch: reference("fixed"),
                },
                PatchChange::Removed {
                    patch: reference("removed"),
                },
                PatchChange::Added {
                    patch: reference("added"),
                },
            ],
        ),
    ] {
        let found = changes(&ChangesRequest {
            record: record.clone(),
            patches,
        })
        .unwrap();
        assert_eq!(found, expected);
    }
}

#[test]
fn a_build_without_a_patch_turned_off_has_no_changes() {
    let tmp = ScratchDir::new("sdk-record").unwrap();
    let key = SigningKey::from_bytes(&[42; 32]);
    let patch = |id: &str| {
        format!(
            r#"
[[patches]]
bundle = "example"
id = "{id}"
name = "{id}"
description = "{id}"
hidden = false
enabled_by_default = true
dependencies = []
options = []

[patches.compatibility]
kind = "packages"
packages = [{{ package = "com.example.test", versions = [] }}]
"#
        )
    };
    let manifest = catalog_manifest(
        &[("kept", &"11".repeat(32)), ("turned.off", &"22".repeat(32))],
        &(patch("kept") + &patch("turned.off")),
    );
    let bundle = tmp.path().join("example.reseam");
    write_catalog(&bundle, &manifest, &manifest, &key);
    let input = tmp.path().join("app.apk");
    std::fs::write(&input, apk("")).unwrap();
    let trust = Trust {
        keys: vec![hex::encode(key.verifying_key().to_bytes())],
    };
    let outcome = crate::patch(
        &PatchRequest {
            apk_path: input.display().to_string(),
            split_paths: Vec::new(),
            bundle_paths: vec![bundle.display().to_string()],
            trust: trust.clone(),
            selection: PatchSelection {
                disable: vec!["example/turned.off".to_owned()],
                ..PatchSelection::default()
            },
            output: PatchOutput::SingleFile {
                path: tmp.path().join("out.apk").display().to_string(),
            },
            signing: None,
            dry_run: true,
            install_method: InstallMethod::Install,
            leave_out_failed: false,
            trace: false,
        },
        |_| {},
    )
    .unwrap();
    let patches = crate::inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![bundle.display().to_string()],
        trust,
    })
    .unwrap()
    .patches;
    assert_eq!(
        changes(&ChangesRequest {
            record: outcome.record,
            patches,
        })
        .unwrap(),
        []
    );
}
