// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::Write;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use std::collections::BTreeMap;
use std::path::Path;

use reseam_model::Problem;

use super::manifest::BundleManifest;
use super::*;
use crate::error::PatcherError;

mod signing;

pub(super) fn load_all(archive: BundleArchive) -> crate::error::Result<PatchBundle> {
    let ids = archive
        .patches()
        .iter()
        .map(|spec| spec.reference().id.clone())
        .collect();
    archive.load(&ids)
}

#[test]
#[expect(
    clippy::used_underscore_binding,
    reason = "inspect the lifetime-owned extracted payload after public loading"
)]
fn signed_bundles_keep_their_resources_and_version() {
    let stage = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(stage.path().join("resources/drawable")).unwrap();
    std::fs::write(
        stage.path().join("manifest.toml"),
        "[bundle]\nname = 'example'\nformat_version = 1\n",
    )
    .unwrap();
    let resource = vec![0x5a; 200_000];
    std::fs::write(stage.path().join("resources/drawable/icon.png"), &resource).unwrap();
    let out = stage.path().join("example.reseam");
    pack(
        stage.path(),
        Some("1.2.0"),
        Some(&test_key(stage.path())),
        &out,
    )
    .unwrap();
    let archive = BundleArchive::open(&out).unwrap();
    assert_eq!(archive.info().version, "1.2.0");
    assert_eq!(
        archive.files().collect::<Vec<_>>(),
        ["resources/drawable/icon.png"]
    );
    let bundle = load_all(archive).unwrap();
    assert_eq!(
        std::fs::read(bundle._extracted.path().join("resources/drawable/icon.png")).unwrap(),
        resource
    );
}

pub(super) fn resource_archive(path: &Path, name: &str, declared: &[u8], actual: &[u8]) {
    signed_archive(path, &resource_manifest(name, declared), name, actual);
}

/// The manifest of a bundle whose only payload file is `name`, holding `declared`.
pub(super) fn resource_manifest(name: &str, declared: &[u8]) -> String {
    let manifest = BundleManifest {
        bundle: BundleInfo {
            name: "example".into(),
            author: String::new(),
            description: String::new(),
            license: String::new(),
            format_version: BUNDLE_FORMAT_VERSION,
            version: String::new(),
            engine: ENGINE_VERSION.into(),
        },
        files: BTreeMap::from([(name.into(), hex::encode(Sha256::digest(declared)))]),
        patches: Vec::new(),
        fingerprints: BTreeMap::new(),
    };
    toml::to_string(&manifest).unwrap()
}

/// The key `signed_archive` signs with, as an author's key file.
pub(super) fn test_key(dir: &Path) -> BundleKey {
    let path = dir.join("bundle-signing.key");
    std::fs::write(&path, [42; 32]).unwrap();
    BundleKey::read(&path).unwrap()
}

fn signed_archive(path: &Path, manifest: &str, name: &str, actual: &[u8]) {
    let key = SigningKey::from_bytes(&[42; 32]);
    write_archive(
        path,
        manifest,
        &[
            ("manifest.pubkey", &key.verifying_key().to_bytes()),
            ("manifest.sig", &key.sign(manifest.as_bytes()).to_bytes()),
            (name, actual),
        ],
    );
}

/// A bundle with `manifest` and then `entries`.
pub(super) fn write_archive(path: &Path, manifest: &str, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in [
        ("mimetype", BUNDLE_MIMETYPE.as_bytes()),
        ("manifest.toml", manifest.as_bytes()),
    ]
    .iter()
    .chain(entries)
    {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn compatibility_errors_take_precedence_over_catalog_schema_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("example.reseam");
    for (engine, older) in [("0.16.0", true), ("1.0.0", false)] {
        let manifest =
            format!("[bundle]\nname = 'example'\nformat_version = 1\nengine = '{engine}'\n");
        signed_archive(&path, &manifest, "resources/icon.png", b"resource");
        let error = BundleArchive::open(&path).err().unwrap();
        assert!(
            if older {
                matches!(error, PatcherError::Problem(Problem::BundleTooOld { .. }))
            } else {
                matches!(error, PatcherError::Problem(Problem::EngineTooOld { .. }))
            },
            "{error}"
        );
    }
    let manifest =
        format!("[bundle]\nname = 'example'\nformat_version = 1\nengine = '{ENGINE_VERSION}'\n");
    signed_archive(&path, &manifest, "resources/icon.png", b"resource");
    assert!(BundleArchive::open(&path).is_err());
}

#[test]
fn rejects_unverified_or_unsafe_resources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("example.reseam");
    for name in [
        "resources/../escape",
        "resources//icon",
        "resources/C:icon",
        "resources/CON.png",
        "resources/a\\b",
        "resources/a.",
    ] {
        resource_archive(&path, name, b"resource", b"resource");
        assert!(
            load_all(BundleArchive::open(&path).unwrap()).is_err(),
            "{name}"
        );
    }
    resource_archive(
        &path,
        "resources/drawable/icon.png",
        b"expected",
        b"tampered",
    );
    assert!(load_all(BundleArchive::open(&path).unwrap()).is_err());
}

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
mod indexed;

#[cfg(feature = "kotlin")]
#[test]
fn the_module_list_round_trips_the_fixture_the_gradle_plugin_writes() {
    let fixture = include_str!("../../../tests/fixtures/bundle-modules.json").trim();
    let modules: Vec<modules::Module> = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_string(&modules).unwrap(), fixture);
}

#[cfg(feature = "bridge")]
#[test]
fn the_patch_index_round_trips_the_fixture_the_gradle_plugin_writes() {
    let fixture = include_str!("../../../tests/fixtures/patch-index.json").trim();
    let declarations: Vec<index::Declaration> = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_string(&declarations).unwrap(), fixture);
}
