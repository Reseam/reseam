// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn write_apk(path: &Path, manifest: &[u8], extra_entries: &[(&str, &[u8])]) {
    let entries: Vec<_> = std::iter::once(("AndroidManifest.xml", manifest))
        .chain(extra_entries.iter().copied())
        .collect();
    write_with_options(
        path,
        &entries,
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    );
}

pub fn write_with_options(
    path: &Path,
    entries: &[(&str, &[u8])],
    options: zip::write::SimpleFileOptions,
) {
    let mut writer = zip::ZipWriter::new(File::create(path).expect("fixture APK"));
    for (name, bytes) in entries {
        writer.start_file(*name, options).expect("fixture entry");
        writer.write_all(bytes).expect("fixture bytes");
    }
    writer.finish().expect("fixture archive");
}
