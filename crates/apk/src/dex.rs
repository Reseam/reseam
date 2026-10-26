// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use rayon::prelude::*;
use reseam_dex::{DexFile, ParseOptions};
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

pub(crate) fn parse_entry(
    mapped: reseam_storage::MappedFile,
    opts: ParseOptions,
) -> Result<Vec<DexFile>> {
    let members = reseam_dex::parse_container_with_bytes(Bytes::from_mmap(Arc::new(mapped)), opts)?;
    if let Some(member) = members.first() {
        member.release_pages();
    }
    Ok(members)
}
