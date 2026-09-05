// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::sync::Arc;

use rayon::prelude::*;
use reseam_dex::file::DexBytes;
use reseam_dex::{DexFile, MultiDexContainer, ParseOptions};

use crate::error::Result;
use crate::zip::reader::{self, Archive};

/// Parses every `classes*.dex` entry of `archive`, in DEX order. Entries are
/// inflated in parallel into file-backed mappings.
pub(crate) fn load_dex(archive: &Archive, opts: &ParseOptions) -> Result<Vec<(String, DexFile)>> {
    reader::dex_entry_names(archive)
        .into_par_iter()
        .map(|name| {
            let mapped = reader::map_entry(&mut archive.clone(), name.as_str())?;
            let dex = reseam_dex::parse_bytes(DexBytes::from_mmap(Arc::new(mapped)), opts.clone())?;
            Ok((name, dex))
        })
        .collect()
}

pub fn extract_dex(path: &Path, opts: ParseOptions) -> Result<(MultiDexContainer, Vec<String>)> {
    let archive = reader::open_archive(path)?;
    let mut container = MultiDexContainer::new();
    let mut names = Vec::new();
    for (name, dex) in load_dex(&archive, &opts)? {
        names.push(name);
        container.add_dex(dex);
    }
    Ok((container, names))
}
