// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::manifest::manifest_bytes;
use super::tables::{package, strings};
use reseam_apk::{ApkFile, ResourceTable};
use reseam_dex::ParseOptions;
use std::fs::File;
use std::path::Path;
pub const ALIGNMENT_DEFAULT: u64 = 4;
pub const ALIGNMENT_NATIVE_LIB: u64 = 16 * 1024;

pub fn lazy() -> ParseOptions {
    ParseOptions {
        classes: reseam_dex::Loading::Deferred,
        ..ParseOptions::default()
    }
}

/// A table without packages, which nothing can be added to.
pub fn resource_table_bytes() -> Vec<u8> {
    ResourceTable::new(strings(&[]), Vec::new())
        .serialize()
        .expect("serialize resources")
}

/// A table with one empty package.
pub fn mutable_resource_table_bytes() -> Vec<u8> {
    ResourceTable::new(
        strings(&[]),
        vec![package("com.example.test", &[], &[], [])],
    )
    .serialize()
    .expect("serialize mutable resources")
}

pub fn entry_data_start_and_compression(
    apk_path: &Path,
    entry_name: &str,
) -> (u64, zip::CompressionMethod) {
    let file = File::open(apk_path).expect("open apk");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let entry = archive.by_name(entry_name).expect("entry");
    (
        entry.data_start().expect("entry offset"),
        entry.compression(),
    )
}

pub struct Session {
    pub dir: tempfile::TempDir,
    pub apk: ApkFile,
}

impl Session {
    pub fn single(entries: &[(&str, &[u8])]) -> Self {
        let dir = tempfile::tempdir().expect("fixture directory");
        std::fs::write(
            dir.path().join("app.apk"),
            reseam_fixtures::apk(&manifest_bytes("original", None), entries),
        )
        .expect("fixture APK");
        let apk = ApkFile::open(dir.path().join("app.apk"), lazy()).expect("fixture APK");
        Self { dir, apk }
    }

    pub fn split(base: &[(&str, &[u8])], split: &[(&str, &[u8])], options: ParseOptions) -> Self {
        let dir = tempfile::tempdir().expect("fixture directory");
        std::fs::write(
            dir.path().join("base.apk"),
            reseam_fixtures::apk(&manifest_bytes("1.0-base", None), base),
        )
        .expect("fixture APK");
        std::fs::write(
            dir.path().join("split.apk"),
            reseam_fixtures::apk(&manifest_bytes("1.0-split", Some("feature")), split),
        )
        .expect("fixture APK");
        let apk = ApkFile::open_split(
            dir.path().join("base.apk"),
            &[dir.path().join("split.apk")],
            options,
        )
        .expect("fixture split APK");
        Self { dir, apk }
    }
}

pub const LIBRARIES: [(&str, u64, &[u8]); 4] = [
    (
        "lib/arm64-v8a/liboriginal.so",
        ALIGNMENT_NATIVE_LIB,
        b"\x7fELF original".as_slice(),
    ),
    (
        "lib/arm64-v8a/libinjected.so",
        ALIGNMENT_NATIVE_LIB,
        b"\x7fELF injected".as_slice(),
    ),
    (
        "lib/arm64-v8a/.so",
        ALIGNMENT_NATIVE_LIB,
        b"\x7fELF injected".as_slice(),
    ),
    ("assets/raw.bin", ALIGNMENT_DEFAULT, b"asset".as_slice()),
];

pub fn component_state(
    apk: &mut ApkFile,
    index: usize,
) -> reseam_apk::Result<(Option<String>, bool)> {
    let component = apk.component_mut(index).expect("fixture component");
    let version = component
        .manifest()
        .version_name()
        .map(std::borrow::Cow::into_owned);
    Ok((version, component.resources()?.is_some()))
}

pub const SIGNATURES: [&str; 5] = [
    "META-INF/MANIFEST.MF",
    "META-INF/CERT.SF",
    "META-INF/CERT.RSA",
    "META-INF/CERT.DSA",
    "meta-inf/cert.ec",
];
pub const METADATA: [&str; 3] = [
    "META-INF/README",
    "META-INF/nested/CERT.SF",
    "assets/data.txt",
];
