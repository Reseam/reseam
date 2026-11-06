// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs;

use reseam_dex::{AccessFlags, DexFile, DexHeader, DexVersion};
use reseam_patcher::bundle::{BUNDLE_FORMAT_VERSION, BundleArchive, BundleKey, pack};

use crate::common::bundles::{TEST_SIGNING_SEED, build_fixture_jar};
use crate::common::code::dex_bytes;

const PATCHES: &str = "reseam-test-patches.jar";

fn extension(class: &str) -> Vec<u8> {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    dex.create_class(class, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
        .unwrap();
    dex_bytes(&dex)
}

/// The patch jar's module uses `used.dex`, `library.dex` stands for its source,
/// and `other.dex` belongs to a module the patches don't use.
fn modules(source: &str) -> String {
    format!(
        r#"[
            {{"name": ":patches", "files": ["{PATCHES}"], "uses": [":used", "library"]}},
            {{"name": ":used", "files": ["used.dex"]}},
            {{"name": "library", "files": ["library.dex"], "source": "{source}"}},
            {{"name": ":other", "files": ["other.dex"]}}
        ]"#
    )
}

/// Packs the fixture patches with `extensions` and `modules`, then reads each patch's fingerprint.
fn fingerprints(
    extensions: &[(&str, Vec<u8>)],
    modules: Option<String>,
) -> reseam_patcher::error::Result<BTreeMap<String, String>> {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(build_fixture_jar(), dir.path().join(PATCHES)).unwrap();
    for (name, bytes) in extensions {
        fs::write(dir.path().join(name), bytes).unwrap();
    }
    if let Some(modules) = modules {
        fs::write(dir.path().join("modules.json"), modules).unwrap();
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
    )?;
    let archive = BundleArchive::open(&out).unwrap();
    Ok(archive
        .patches()
        .iter()
        .map(|patch| (patch.id.clone(), archive.fingerprint(patch).to_owned()))
        .collect())
}

const SOURCE: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OTHER_SOURCE: &str = "2222222222222222222222222222222222222222222222222222222222222222";

#[test]
fn a_fingerprint_follows_the_files_of_the_modules_its_patch_uses() {
    let packed = |used: &str, library: &str, other: &str, source: &str| {
        fingerprints(
            &[
                ("used.dex", extension(used)),
                ("library.dex", extension(library)),
                ("other.dex", extension(other)),
            ],
            Some(modules(source)),
        )
        .unwrap()
    };
    let before = packed("LUsed;", "LLibrary;", "LOther;", SOURCE);
    assert_eq!(
        packed("LUsed;", "LLibrary;", "LOther;", SOURCE),
        before,
        "packing again gives the same fingerprints"
    );
    for (case, after, changes) in [
        (
            "a used module's file",
            packed("LUsed2;", "LLibrary;", "LOther;", SOURCE),
            true,
        ),
        (
            "a module the patches don't use",
            packed("LUsed;", "LLibrary;", "LOther2;", SOURCE),
            false,
        ),
        (
            "a file a module's source stands for",
            packed("LUsed;", "LLibrary2;", "LOther;", SOURCE),
            false,
        ),
        (
            "a used module's source",
            packed("LUsed;", "LLibrary;", "LOther;", OTHER_SOURCE),
            true,
        ),
    ] {
        assert!(!before.is_empty());
        for (id, fingerprint) in &before {
            assert_eq!(after[id] != *fingerprint, changes, "{case} changes {id}");
        }
    }
}

#[test]
fn without_modules_any_payload_file_changes_every_fingerprint() {
    let without = fingerprints(&[], None).unwrap();
    let with_file = fingerprints(&[("other.dex", extension("LOther;"))], None).unwrap();
    assert!(!without.is_empty());
    for (id, fingerprint) in &without {
        assert_ne!(with_file[id], *fingerprint, "{id}");
    }
}

#[test]
fn packing_refuses_modules_that_do_not_match_the_payload() {
    let extensions = [("used.dex", extension("LUsed;"))];
    for (case, modules) in [
        (
            "a file that is not in the payload",
            format!(
                r#"[{{"name": ":patches", "files": ["{PATCHES}", "used.dex", "missing.dex"]}}]"#
            ),
        ),
        (
            "a payload file in no module",
            format!(r#"[{{"name": ":patches", "files": ["{PATCHES}"]}}]"#),
        ),
        (
            "a file in two modules",
            format!(
                r#"[{{"name": ":patches", "files": ["{PATCHES}", "used.dex"]}}, {{"name": ":used", "files": ["used.dex"]}}]"#
            ),
        ),
        (
            "a module listed twice",
            format!(
                r#"[{{"name": ":patches", "files": ["{PATCHES}"]}}, {{"name": ":patches", "files": ["used.dex"]}}]"#
            ),
        ),
        (
            "a use of an unknown module",
            format!(
                r#"[{{"name": ":patches", "files": ["{PATCHES}", "used.dex"], "uses": [":gone"]}}]"#
            ),
        ),
        (
            "a source that is not a SHA-256",
            format!(
                r#"[{{"name": ":patches", "files": ["{PATCHES}"]}}, {{"name": ":used", "files": ["used.dex"], "source": "abc"}}]"#
            ),
        ),
    ] {
        assert!(fingerprints(&extensions, Some(modules)).is_err(), "{case}");
    }
}
