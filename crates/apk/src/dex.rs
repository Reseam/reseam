// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::sync::Arc;

use rayon::prelude::*;
use reseam_dex::{DexFile, MultiDexContainer, ParseOptions};
use reseam_storage::Bytes;

use crate::entry::EntryName;
use crate::error::Result;
use crate::zip::reader::{self, Archive};

pub(crate) fn load_dex(archive: &Archive, opts: ParseOptions) -> Result<Vec<(EntryName, DexFile)>> {
    reader::dex_entry_names(archive)
        .into_par_iter()
        .map(|name| {
            let mapped = reader::map_entry(&mut archive.clone(), name.as_str())
                .map_err(|error| error.in_entry(&name))?;
            let dex = parse_entry(mapped, opts).map_err(|error| error.in_entry(&name))?;
            Ok((EntryName::from(name), dex))
        })
        .collect::<Result<Vec<_>>>()
        .map(|entries| {
            entries
                .into_iter()
                .flat_map(|(name, members)| members.into_iter().map(move |dex| (name.clone(), dex)))
                .collect()
        })
}

pub(crate) fn parse_entry(mapped: memmap2::Mmap, opts: ParseOptions) -> Result<Vec<DexFile>> {
    let members = reseam_dex::parse_container_with_bytes(Bytes::from_mmap(Arc::new(mapped)), opts)?;
    if let Some(member) = members.first() {
        member.release_pages();
    }
    Ok(members)
}

/// Loads logical DEX files in entry and container-member order, using shared
/// file-backed storage. Each name identifies the physical ZIP entry, so v041
/// containers repeat their entry name for every member. Invalid members fail
/// extraction rather than returning a partial container.
pub fn extract_dex(path: &Path, opts: ParseOptions) -> Result<(MultiDexContainer, Vec<String>)> {
    let archive = reader::open_archive(path)?;
    let mut container = MultiDexContainer::new();
    let mut names = Vec::new();
    for (name, dex) in load_dex(&archive, opts)? {
        names.push(name.to_string());
        container.add_dex(dex);
    }
    Ok((container, names))
}
