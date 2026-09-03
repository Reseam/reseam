// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lazy-aware writer must produce output equivalent to the eager writer.
//! A DEX parsed lazily (classes never materialized) is written by copying
//! each file class straight from the raw buffer with its pool indices
//! rewritten; the same DEX with every class resolved is decoded, remapped
//! and re-encoded. Parsed back, both outputs must hold the same pools and
//! the same classes, method for method.
//!
//! Two paths matter:
//! - identity sort: an unmodified, canonically-sorted DEX (no pool reorder);
//! - remap sort: a pool perturbation (a new string) forces every operand to be
//!   remapped, so the raw copier must apply exactly the remap the resident
//!   path applies.

use std::io::Read;

use reseam_dex::ParseOptions;

const APK: &str = "../../test-apks/com.google.android.youtube_20.40.45.apk";

fn dexes() -> Vec<Vec<u8>> {
    let Ok(apk_bytes) = std::fs::read(APK) else {
        return Vec::new();
    };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(apk_bytes)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let name = entry.name().to_owned();
        if name.starts_with("classes") && name.ends_with(".dex") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            out.push(buf);
        }
    }
    out
}

fn lazy_opts() -> ParseOptions {
    ParseOptions {
        lazy: true,
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    }
}

/// Writes the DEX with every class resolved up front (eager path).
fn eager_write(dex_bytes: &[u8], perturb: bool) -> Vec<u8> {
    let mut dex = reseam_dex::parse(dex_bytes, lazy_opts()).expect("parse");
    dex.resolve_all_class_data().expect("resolve all");
    if perturb {
        dex.strings.push("!!lazy_write_marker");
    }
    reseam_dex::write(&dex).expect("eager write")
}

/// Writes the DEX with classes left in the file (raw copy path).
fn lazy_write(dex_bytes: &[u8], perturb: bool) -> Vec<u8> {
    let mut dex = reseam_dex::parse(dex_bytes, lazy_opts()).expect("parse");
    if perturb {
        dex.strings.push("!!lazy_write_marker");
    }
    reseam_dex::write(&dex).expect("lazy write")
}

/// Everything a DEX holds, fully decoded, for comparing two writers' output.
struct Snapshot {
    strings: Vec<String>,
    prototypes: Vec<reseam_dex::Prototype>,
    fields: Vec<reseam_dex::FieldId>,
    methods: Vec<reseam_dex::MethodId>,
    classes: Vec<reseam_dex::ClassDef>,
}

fn snapshot(bytes: &[u8]) -> Snapshot {
    let mut dex = reseam_dex::parse(bytes, lazy_opts()).expect("parse output");
    dex.resolve_all_class_data().expect("resolve output");
    Snapshot {
        strings: dex.strings.iter().map(|s| s.into_owned()).collect(),
        prototypes: dex.prototypes.to_vec(),
        fields: dex.fields.to_vec(),
        methods: dex.methods.to_vec(),
        classes: dex.classes.iter_resident().cloned().collect(),
    }
}

#[test]
fn lazy_write_is_equivalent_to_eager_write() {
    let dexes = dexes();
    if dexes.is_empty() {
        eprintln!("Skipping: test APK not found at {APK}");
        return;
    }

    for (i, dex_bytes) in dexes.iter().enumerate() {
        for perturb in [false, true] {
            let eager = snapshot(&eager_write(dex_bytes, perturb));
            let lazy = snapshot(&lazy_write(dex_bytes, perturb));
            assert_eq!(lazy.strings, eager.strings, "strings differ (perturb={perturb}) for classes{}.dex", i + 1);
            assert_eq!(lazy.prototypes, eager.prototypes, "protos differ (perturb={perturb}) for classes{}.dex", i + 1);
            assert_eq!(lazy.fields, eager.fields, "fields differ (perturb={perturb}) for classes{}.dex", i + 1);
            assert_eq!(lazy.methods, eager.methods, "methods differ (perturb={perturb}) for classes{}.dex", i + 1);
            assert_eq!(lazy.classes.len(), eager.classes.len());
            for (l, e) in lazy.classes.iter().zip(&eager.classes) {
                assert_eq!(l, e, "class differs (perturb={perturb}) for classes{}.dex", i + 1);
            }
        }
    }

    eprintln!("verified lazy write == eager write across {} dexes", dexes.len());
}

#[test]
fn spooled_write_is_byte_identical_to_memory_write() {
    let dexes = dexes();
    if dexes.is_empty() {
        eprintln!("Skipping: test APK not found at {APK}");
        return;
    }
    for (i, bytes) in dexes.iter().enumerate() {
        for perturb in [false, true] {
            let mut memory = reseam_dex::parse(bytes, lazy_opts()).expect("parse");
            let mut spool = reseam_dex::parse(bytes, lazy_opts()).expect("parse");
            if perturb {
                memory.strings.push("!!lazy_write_marker");
                spool.strings.push("!!lazy_write_marker");
            }
            let expected = reseam_dex::write(&memory).expect("memory write");
            let spooled = reseam_dex::write_spooled(&spool).expect("spooled write");
            assert_eq!(spooled.len(), expected.len() as u64, "dex {i} perturb={perturb}");
            assert!(
                spooled.map().expect("map")[..] == expected[..],
                "dex {i} perturb={perturb}: spooled bytes differ"
            );
        }
    }
}
