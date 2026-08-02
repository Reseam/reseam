// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transient per-method decode (used by read-only patch inspection) must return
//! exactly what full class resolution produces, for both the raw and the
//! already-materialized code paths.

use std::io::Read;

use reseam_dex::ParseOptions;

const APK: &str = "../../test-apks/com.google.android.youtube_20.40.45.apk";

fn first_dex() -> Option<Vec<u8>> {
    let apk_bytes = std::fs::read(APK).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(apk_bytes)).ok()?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).ok()?;
        let name = entry.name().to_owned();
        if name.starts_with("classes") && name.ends_with(".dex") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

fn lazy_opts() -> ParseOptions {
    ParseOptions {
        lazy: true,
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    }
}

#[test]
fn transient_decode_matches_full_resolution() {
    let Some(dex_bytes) = first_dex() else {
        eprintln!("Skipping: test APK not found at {APK}");
        return;
    };

    // Raw path: transient decode against a still-deferred class.
    let lazy = reseam_dex::parse(&dex_bytes, lazy_opts()).expect("lazy parse");
    // Fully-resolved reference.
    let mut resolved = reseam_dex::parse(&dex_bytes, lazy_opts()).expect("lazy parse");
    resolved.resolve_all_class_data().expect("resolve all");

    let mut checked_methods = 0usize;
    let mut checked_raw = 0usize;

    for class_idx in 0..resolved.classes.len() {
        let Some(data) = resolved.classes[class_idx].class_data.as_ref() else {
            continue;
        };
        assert!(
            lazy.classes[class_idx].class_data.is_none(),
            "lazy dex should not have materialized class {class_idx}"
        );

        for (is_virtual, methods) in [
            (false, &data.direct_methods),
            (true, &data.virtual_methods),
        ] {
            for (method_pos, expected) in methods.iter().enumerate() {
                let transient = lazy
                    .decode_method_at(class_idx, method_pos, is_virtual)
                    .expect("transient decode")
                    .expect("method present");

                assert_eq!(transient.method, expected.method);
                assert_eq!(transient.access_flags, expected.access_flags);
                assert_eq!(
                    transient.code.as_ref().map(|c| c.instructions.clone()),
                    expected.code.as_ref().map(|c| c.instructions.clone()),
                    "instructions differ at class {class_idx} method {method_pos}"
                );
                checked_methods += 1;
            }
        }

        // Cap the sweep so the test stays fast on a real APK.
        if class_idx > 2000 {
            break;
        }
    }

    // Materialized path: decoding an already-resolved class clones the same method.
    for class_idx in 0..resolved.classes.len() {
        let Some(data) = resolved.classes[class_idx].class_data.as_ref() else {
            continue;
        };
        if let Some(expected) = data.direct_methods.first() {
            let cloned = resolved
                .decode_method_at(class_idx, 0, false)
                .expect("clone decode")
                .expect("method present");
            assert_eq!(cloned.method, expected.method);
            assert_eq!(
                cloned.code.as_ref().map(|c| c.instructions.len()),
                expected.code.as_ref().map(|c| c.instructions.len())
            );
            checked_raw += 1;
            if checked_raw > 500 {
                break;
            }
        }
    }

    assert!(checked_methods > 0, "no methods checked");
    eprintln!("verified {checked_methods} transient decodes match full resolution");
}
