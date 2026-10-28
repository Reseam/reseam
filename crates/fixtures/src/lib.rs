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

/// `apk` with an APK Signing Block holding `pairs` before its central
/// directory, as a store's signer leaves one. `apk` must have no ZIP comment.
pub fn with_signing_block(apk: &[u8], pairs: &[(u32, &[u8])]) -> Vec<u8> {
    let eocd = apk.len() - 22;
    assert!(
        apk[eocd..].starts_with(b"PK\x05\x06"),
        "fixture APK has a ZIP comment"
    );
    let cd_offset = u32::from_le_bytes(apk[eocd + 16..eocd + 20].try_into().expect("u32")) as usize;
    let pairs: Vec<u8> = pairs
        .iter()
        .flat_map(|(id, value)| {
            [
                &(value.len() as u64 + 4).to_le_bytes()[..],
                &id.to_le_bytes(),
                value,
            ]
            .concat()
        })
        .collect();
    let size = (pairs.len() as u64 + 24).to_le_bytes();
    let block = [&size[..], &pairs, &size, b"APK Sig Block 42"].concat();
    let mut output = [&apk[..cd_offset], &block, &apk[cd_offset..]].concat();
    let field = eocd + block.len() + 16;
    output[field..field + 4].copy_from_slice(&((cd_offset + block.len()) as u32).to_le_bytes());
    output
}

/// `zip` as tools that predate the UTF-8 flag write it: every entry's language encoding flag is
/// cleared and its name bytes are unchanged. `zip` must have no ZIP comment.
pub fn without_utf8_flags(zip: &[u8]) -> Vec<u8> {
    const UTF8_NAME: u16 = 1 << 11;
    let u16_at = |bytes: &[u8], at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |bytes: &[u8], at: usize| {
        u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32")) as usize
    };
    let eocd = zip.len() - 22;
    assert!(
        zip[eocd..].starts_with(b"PK\x05\x06"),
        "fixture ZIP has a ZIP comment"
    );
    let mut output = zip.to_vec();
    let mut record = u32_at(zip, eocd + 16);
    for _ in 0..u16_at(zip, eocd + 10) {
        for flags in [record + 8, u32_at(zip, record + 42) + 6] {
            let cleared = u16_at(zip, flags) & !UTF8_NAME;
            output[flags..flags + 2].copy_from_slice(&cleared.to_le_bytes());
        }
        record += 46
            + u16_at(zip, record + 28) as usize
            + u16_at(zip, record + 30) as usize
            + u16_at(zip, record + 32) as usize;
    }
    output
}
