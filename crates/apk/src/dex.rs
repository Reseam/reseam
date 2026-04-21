// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::Result;
use crate::zip::reader::ApkReader;
use reseam_dex::{MultiDexContainer, ParseOptions};
use std::io::{Read, Seek};
use tracing::{debug, instrument};

/// Extract and parse all DEX files from a single APK reader.
#[instrument(level = "debug", skip(reader), fields(lazy = opts.lazy))]
pub fn extract_dex<R: Read + Seek>(
    reader: &mut ApkReader<R>,
    opts: ParseOptions,
) -> Result<MultiDexContainer> {
    let mut container = MultiDexContainer::new();
    append_reader_dex(reader, &mut container, &opts)?;
    debug!(dex_count = container.len(), "extracted DEX files from APK");
    Ok(container)
}

/// Extract DEX from multiple APK readers (base + splits) into a unified container.
///
/// All DEX from all APKs is merged into a single `MultiDexContainer`.
/// Config splits with no DEX are silently skipped.
#[instrument(level = "debug", skip(readers), fields(reader_count = readers.len(), lazy = opts.lazy))]
pub fn extract_dex_unified<R: Read + Seek>(
    readers: &mut [&mut ApkReader<R>],
    opts: ParseOptions,
) -> Result<MultiDexContainer> {
    let mut container = MultiDexContainer::new();
    for reader in readers.iter_mut() {
        append_reader_dex(reader, &mut container, &opts)?;
    }
    debug!(
        dex_count = container.len(),
        "extracted unified DEX container"
    );
    Ok(container)
}

/// Serialize all DEX files in a container to named entries for writing into an APK.
///
/// Returns entries named `classes.dex`, `classes2.dex`, `classes3.dex`, etc.
#[instrument(level = "debug", skip(container), fields(dex_count = container.len()))]
pub fn dex_to_entries(container: &mut MultiDexContainer) -> Result<Vec<(String, Vec<u8>)>> {
    let buffers = container.write_all()?;
    let mut entries = Vec::with_capacity(buffers.len());
    for (i, buf) in buffers.into_iter().enumerate() {
        let name = if i == 0 {
            "classes.dex".to_string()
        } else {
            format!("classes{}.dex", i + 1)
        };
        entries.push((name, buf));
    }
    Ok(entries)
}

/// Extracts and parses `classes*.dex` entries from an APK or ZIP byte slice.
///
/// Convenience function for loading DEX from raw APK bytes without constructing an `ApkReader`.
#[instrument(level = "debug", skip(apk_bytes), fields(apk_size = apk_bytes.len(), lazy = opts.lazy))]
pub fn from_apk(apk_bytes: &[u8], opts: ParseOptions) -> Result<MultiDexContainer> {
    use std::io::Cursor;

    let reader = Cursor::new(apk_bytes);
    let mut archive = ::zip::ZipArchive::new(reader)?;

    let mut dex_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name == "classes.dex" || (name.starts_with("classes") && name.ends_with(".dex")) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    dex_names.sort_by_key(|a| dex_sort_key(a));

    let mut dex_files = Vec::with_capacity(dex_names.len());
    for name in &dex_names {
        let mut entry = archive.by_name(name)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        dex_files.push(reseam_dex::parse_owned(buf, opts.clone())?);
    }

    let mut container = MultiDexContainer::new();
    for dex in dex_files {
        container.add_dex(dex);
    }
    debug!(
        dex_count = container.len(),
        "parsed DEX entries from APK bytes"
    );
    Ok(container)
}

fn append_reader_dex<R: Read + Seek>(
    reader: &mut ApkReader<R>,
    container: &mut MultiDexContainer,
    opts: &ParseOptions,
) -> Result<()> {
    let dex_names = reader.dex_entry_names();
    for name in dex_names {
        let buffer = reader.read_entry(&name)?;
        container.add_dex(reseam_dex::parse_owned(buffer, opts.clone())?);
    }
    Ok(())
}

/// Sort key for DEX entry names. Android convention:
/// `classes.dex` (primary) < `classes2.dex` < `classes3.dex` < ...
pub(crate) fn dex_sort_key(name: &str) -> u32 {
    match name {
        "classes.dex" => 0,
        _ => name
            .strip_prefix("classes")
            .and_then(|s| s.strip_suffix(".dex"))
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(u32::MAX),
    }
}
