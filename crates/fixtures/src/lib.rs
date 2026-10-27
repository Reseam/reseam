// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Archives the workspace's tests build as input.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// An APK of `manifest`, as its compiled `AndroidManifest.xml`, followed by `entries`.
pub fn apk(manifest: &[u8], entries: &[(&str, &[u8])]) -> Vec<u8> {
    let entries: Vec<_> = std::iter::once(("AndroidManifest.xml", manifest))
        .chain(entries.iter().copied())
        .collect();
    zip(&entries)
}

/// A ZIP of `entries`, stored uncompressed.
pub fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zip_with(
        entries,
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
}

/// A ZIP of `entries`, each written with `options`.
pub fn zip_with(entries: &[(&str, &[u8])], options: SimpleFileOptions) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer.start_file(*name, options).expect("fixture entry");
        writer.write_all(bytes).expect("fixture bytes");
    }
    writer.finish().expect("fixture archive").into_inner()
}
