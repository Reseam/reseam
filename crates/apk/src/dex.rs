// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::sync::Arc;

use rayon::prelude::*;
use reseam_dex::file::DexBytes;
use reseam_dex::{DexFile, MultiDexContainer, ParseOptions};
use tracing::{debug, instrument};

use crate::error::Result;
use crate::zip::reader::{entry_bytes, ApkReader, SharedFile};

/// Extracts and parses every `classes*.dex` entry of the APK at `path`.
///
/// Entries are inflated in parallel from one parsed archive into file-backed
/// mappings; the returned names are in DEX order.
#[instrument(level = "debug", skip_all, fields(apk_path = %path.display(), lazy = opts.lazy))]
pub fn extract_dex(path: &Path, opts: ParseOptions) -> Result<(MultiDexContainer, Vec<String>)> {
    let mut reader = ApkReader::new(SharedFile::open(path)?)?;
    let names = reader.dex_entry_names();
    let archive = reader.into_archive();
    let dexes: Vec<DexFile> = names
        .par_iter()
        .map(|name| {
            let mut archive = archive.clone();
            let file = archive.clone().into_inner();
            let mapped = entry_bytes(file.file(), &mut archive.by_name(name)?)?;
            Ok(reseam_dex::parse_bytes(DexBytes::from_mmap(Arc::new(mapped)), opts.clone())?)
        })
        .collect::<Result<_>>()?;

    let mut container = MultiDexContainer::new();
    for dex in dexes {
        container.add_dex(dex);
    }
    debug!(dex_count = container.len(), "extracted DEX files from APK");
    Ok((container, names))
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
