// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use reseam_model::Problem;

use super::super::{BundleArchive, container, pack, sign};
use super::{load_all, resource_archive, resource_manifest, test_key, write_archive};
use crate::error::{BundleError, PatcherError};

/// Whether an error is the one a case expects.
type Expected = fn(&PatcherError) -> bool;

#[test]
fn a_bundle_packed_without_a_key_opens_once_signed() {
    let stage = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(stage.path().join("resources")).unwrap();
    std::fs::write(
        stage.path().join("manifest.toml"),
        "[bundle]\nname = 'example'\nformat_version = 1\n",
    )
    .unwrap();
    std::fs::write(stage.path().join("resources/icon.png"), b"resource").unwrap();
    let key = test_key(stage.path());
    let [unsigned, signed, packed] =
        ["unsigned", "signed", "packed"].map(|name| stage.path().join(format!("{name}.reseam")));
    pack(stage.path(), Some("1.2.0"), None, &unsigned).unwrap();
    assert!(BundleArchive::open(&unsigned).is_err());
    sign(&unsigned, &key, &signed).unwrap();
    pack(stage.path(), Some("1.2.0"), Some(&key), &packed).unwrap();
    let manifest = |path: &Path| {
        container::read_entry(
            &mut container::open(path).unwrap(),
            "manifest.toml",
            container::MANIFEST_LIMIT,
        )
        .unwrap()
    };
    assert_eq!(manifest(&signed), manifest(&packed));
    let archive = BundleArchive::open(&signed).unwrap();
    assert_eq!(archive.public_key(), &key.public_key());
    assert_eq!(archive.info().version, "1.2.0");
    load_all(archive).unwrap();
}

#[test]
fn signing_refuses_what_opening_would_and_signed_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key(dir.path());
    let input = |case: &str| dir.path().join(format!("{case}.reseam"));
    let (icon, escape) = ("resources/icon.png", "resources/../escape");
    resource_archive(&input("signed"), icon, b"resource", b"resource");
    write_archive(
        &input("changed"),
        &resource_manifest(icon, b"expected"),
        &[(icon, b"tampered")],
    );
    write_archive(
        &input("undeclared"),
        &resource_manifest(icon, b"resource"),
        &[(icon, b"resource"), ("resources/extra.png", b"extra")],
    );
    write_archive(
        &input("unsafe"),
        &resource_manifest(escape, b"resource"),
        &[(escape, b"resource")],
    );
    write_archive(
        &input("newer"),
        "[bundle]\nname = 'example'\nformat_version = 1\nengine = '1.0.0'\n",
        &[],
    );
    let cases: [(&str, Expected); 5] = [
        ("signed", |error| {
            matches!(error, PatcherError::Bundle(BundleError::AlreadySigned(_)))
        }),
        ("changed", |error| {
            matches!(error, PatcherError::Bundle(BundleError::HashMismatch(_)))
        }),
        ("undeclared", |error| {
            matches!(
                error,
                PatcherError::Bundle(BundleError::UndeclaredPayload(_))
            )
        }),
        ("unsafe", |error| {
            matches!(error, PatcherError::Bundle(BundleError::UnsafePath(_)))
        }),
        ("newer", |error| {
            matches!(error, PatcherError::Problem(Problem::EngineTooOld { .. }))
        }),
    ];
    let out = dir.path().join("out.reseam");
    for (case, expected) in cases {
        let error = sign(&input(case), &key, &out).unwrap_err();
        assert!(expected(&error), "{case}: {error}");
        assert!(!out.exists(), "{case}");
    }
}
