// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lazy-aware writer must produce byte-identical output to the eager
//! writer. A DEX parsed lazily (classes never materialized) is written by
//! streaming each deferred class straight from the raw buffer — decoding,
//! remapping, and re-encoding one at a time. That output must equal the output
//! of the same DEX with every class fully resolved before writing.
//!
//! Two paths matter:
//! - identity sort: an unmodified, canonically-sorted DEX (no pool reorder);
//! - remap sort: a pool perturbation (a new string) forces every instruction
//!   operand to be remapped, so the deferred-class emitter must apply the exact
//!   same remap + widening the resident path applies.

use std::io::Read;

use reseam_dex::{DexString, ParseOptions};

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
        dex.strings.push(DexString::new("!!lazy_write_marker"));
    }
    reseam_dex::write(&mut dex).expect("eager write")
}

/// Writes the DEX with classes left deferred (lazy streaming path).
fn lazy_write(dex_bytes: &[u8], perturb: bool) -> Vec<u8> {
    let mut dex = reseam_dex::parse(dex_bytes, lazy_opts()).expect("parse");
    if perturb {
        dex.strings.push(DexString::new("!!lazy_write_marker"));
    }
    reseam_dex::write(&mut dex).expect("lazy write")
}

#[test]
fn lazy_write_is_byte_identical_to_eager_write() {
    let dexes = dexes();
    if dexes.is_empty() {
        eprintln!("Skipping: test APK not found at {APK}");
        return;
    }

    for (i, dex_bytes) in dexes.iter().enumerate() {
        // Identity path: an unmodified, already-sorted DEX.
        let eager = eager_write(dex_bytes, false);
        let lazy = lazy_write(dex_bytes, false);
        assert_eq!(
            lazy, eager,
            "lazy vs eager write differ (identity sort) for classes{}.dex",
            i + 1
        );

        // Remap path: a new string reorders the pools, forcing every deferred
        // class's operands to be remapped by the streaming emitter.
        let eager_p = eager_write(dex_bytes, true);
        let lazy_p = lazy_write(dex_bytes, true);
        assert_eq!(
            lazy_p, eager_p,
            "lazy vs eager write differ (remap sort) for classes{}.dex",
            i + 1
        );
    }

    eprintln!("verified lazy write == eager write across {} dexes", dexes.len());
}
