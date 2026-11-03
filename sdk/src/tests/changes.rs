// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::{
    AppIdentity, BuildRecord, ChangesRequest, InstallMethod, PatchChange, PatchMetadata,
    PatchSelection, RecordedPatch,
};

use super::{metadata, reference, spec};
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
