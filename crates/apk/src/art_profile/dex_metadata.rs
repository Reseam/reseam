// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Cursor;
use std::path::Path;

use tracing::warn;

use super::write::ArtVersion;
use super::{BaselineProfile, METADATA_ENTRY, PROFILE_ENTRY};
use crate::axml::AxmlDocument;
use crate::entry::MANIFEST_ENTRY;
use crate::error::Result;
use crate::zip::Compression;
use crate::zip::reader;
use crate::zip::writer::{DEFAULT_LEVEL, Writer};

/// The dex metadata (`.dm`) file for installing `apk` as the base APK on a
/// device running API level `sdk`. Written into the install session next to
/// the APK, under its name with `.dm` in place of `.apk`, it has ART compile
/// the app with its baseline profile at install instead of only after the
/// device's next idle maintenance. `None` below Android 9, which reads no
/// profile at install, for a split APK, when the APK ships no baseline
/// profile or one this engine cannot read, or on Android 12 and later when it
/// ships no `baseline.profm`.
pub fn dex_metadata(apk: &Path, sdk: u32) -> Result<Option<Vec<u8>>> {
    let version = match sdk {
        ..28 => return Ok(None),
        28..31 => ArtVersion::P,
        _ => ArtVersion::S,
    };
    let mut archive = reader::open_archive(apk)?;
    let manifest = AxmlDocument::parse(&reader::read_entry(&mut archive, MANIFEST_ENTRY)?)?;
    if manifest.split_name().is_some()
        || !reader::contains(&archive, PROFILE_ENTRY)
        || (version == ArtVersion::S && !reader::contains(&archive, METADATA_ENTRY))
    {
        return Ok(None);
    }
    let profile = reader::read_entry(&mut archive, PROFILE_ENTRY)?;
    let metadata = reader::contains(&archive, METADATA_ENTRY)
        .then(|| reader::read_entry(&mut archive, METADATA_ENTRY))
        .transpose()?;
    let primary = match BaselineProfile::parse(&profile, metadata.as_deref())
        .and_then(|profile| profile.to_installed(version))
    {
        Ok(primary) => primary,
        Err(error) => {
            warn!(%error, "no dex metadata for an unreadable baseline profile");
            return Ok(None);
        }
    };
    let mut zip = Writer::new(Cursor::new(Vec::new()), DEFAULT_LEVEL);
    zip.write("primary.prof", primary.as_slice(), Compression::Stored)?;
    Ok(Some(zip.finish()?.into_inner()))
}
