// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use super::BundleKey;
use super::container::{self, MANIFEST_LIMIT, read_entry};
use super::manifest::BundleManifest;
use super::payload::copy_payload;
use crate::error::{BundleError, Result};

/// Signs the unsigned bundle at `unsigned`, made by [`super::pack`] without a
/// key, and writes the signed bundle to `out`. Checks the manifest as opening
/// a bundle does and every payload file against it, and runs none of the
/// bundle's code, so the key can stay off the machine that packed it.
pub fn sign(unsigned: &Path, key: &BundleKey, out: &Path) -> Result<()> {
    let mut archive = container::open(unsigned)?;
    if ["manifest.pubkey", "manifest.sig"]
        .iter()
        .any(|name| archive.index_for_name(name).is_some())
    {
        return Err(BundleError::AlreadySigned(unsigned.to_owned()).into());
    }
    let manifest = read_entry(&mut archive, "manifest.toml", MANIFEST_LIMIT)?;
    let declared = BundleManifest::parse(&manifest)?.files;
    let payload = copy_payload(&mut archive, &declared, |_, _| Ok(std::io::sink()))?;
    container::write(out, &manifest, Some(key), |zip| {
        for (name, _) in &payload {
            let index = archive
                .index_for_name(name)
                .expect("the payload names come from this archive");
            zip.raw_copy_file(archive.by_index_raw(index)?)?;
        }
        Ok(())
    })
}
