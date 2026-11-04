// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs;

use reseam_dex::{AccessFlags, DexFile, DexHeader, DexVersion, EncodedMethod};
use reseam_patcher::bundle::{BUNDLE_FORMAT_VERSION, BundleArchive, BundleKey, pack};

use crate::common::bundles::{TEST_SIGNING_SEED, build_fixture_jar};
use crate::common::code::dex_bytes;

/// Named by the fixture's extension body patches through a binding.
const BODIES: &str = "Lapp/reseam/test/Bodies;";

fn extension(class: &str, flags: AccessFlags) -> Vec<u8> {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    dex.create_class(class, flags, Some("Ljava/lang/Object;"))
        .unwrap();
    dex_bytes(&dex)
}

/// `Bodies` with one native method `entry`, declared with `flags`.
fn bodies_with_entry(flags: AccessFlags) -> Vec<u8> {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    let class = dex
        .create_class(BODIES, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    let entry = dex.intern_method(BODIES, "entry", "()V").unwrap();
    dex.class_mut(class).unwrap().add_method(EncodedMethod {
        method: entry,
        access_flags: flags | AccessFlags::PUBLIC | AccessFlags::NATIVE,
        code: None,
    });
    dex_bytes(&dex)
}

/// Packs the fixture patches with `extensions`, then reads each patch's fingerprint.
fn fingerprints(extensions: &[(&str, Vec<u8>)]) -> BTreeMap<String, String> {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(
        build_fixture_jar(),
        dir.path().join("reseam-test-patches.jar"),
    )
    .unwrap();
    for (name, bytes) in extensions {
        fs::write(dir.path().join(name), bytes).unwrap();
    }
    fs::write(
        dir.path().join("manifest.toml"),
        format!("[bundle]\nname = 'fingerprints'\nformat_version = {BUNDLE_FORMAT_VERSION}\n"),
    )
    .unwrap();
    let key = dir.path().join("bundle-signing.key");
    fs::write(&key, TEST_SIGNING_SEED).unwrap();
    let out = dir.path().join("fingerprints.reseam");
    pack(
        dir.path(),
        None,
        Some(&BundleKey::read(&key).unwrap()),
        &out,
    )
    .unwrap();
    let archive = BundleArchive::open(&out).unwrap();
    archive
        .patches()
        .iter()
        .map(|patch| (patch.id.clone(), archive.fingerprint(patch).to_owned()))
        .collect()
}

#[test]
fn a_changed_extension_class_changes_only_the_patches_that_name_it() {
    let before = fingerprints(&[("bodies.dex", extension(BODIES, AccessFlags::PUBLIC))]);
    let after = fingerprints(&[(
        "bodies.dex",
        extension(BODIES, AccessFlags::PUBLIC | AccessFlags::FINAL),
    )]);
    let changed: Vec<&str> = before
        .iter()
        .filter(|&(id, fingerprint)| after[id] != *fingerprint)
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        changed,
        [
            "app.reseam.test.filledBodies",
            "app.reseam.test.unfilledBodies"
        ]
    );
}

#[test]
fn code_no_patch_reaches_leaves_every_fingerprint_as_it_was() {
    let without = fingerprints(&[]);
    assert_eq!(
        fingerprints(&[]),
        without,
        "packing again gives the same fingerprints"
    );
    let unreached = extension("Lapp/reseam/test/Unreached;", AccessFlags::PUBLIC);
    assert_eq!(fingerprints(&[("unreached.dex", unreached)]), without);
}

#[test]
fn final_changes_a_fingerprint_only_where_a_subclass_could_override() {
    let fingerprints_of = |flags| fingerprints(&[("bodies.dex", bodies_with_entry(flags))]);
    assert_eq!(
        fingerprints_of(AccessFlags::STATIC),
        fingerprints_of(AccessFlags::STATIC | AccessFlags::FINAL),
        "ART ignores final on a static method"
    );
    assert_ne!(
        fingerprints_of(AccessFlags::empty()),
        fingerprints_of(AccessFlags::FINAL)
    );
}
