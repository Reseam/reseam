// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::SECTION;
use super::encode::{deflate, header, u16_len, write_ascending, write_deflated, write_key};
use super::{
    BaselineProfile, DexMetadata, DexProfile, EMBEDDED_ENTRY, METADATA_ENTRY, METADATA_MAGIC,
    METADATA_V002, MethodFlags, PROFILE_ENTRY, PROFILE_MAGIC, PROFILE_V010, PROFILE_V015,
};
use crate::error::{Result, invalid};
use reseam_bytes::{write_u16, write_u32};

const SECTION_DEX_FILES: u32 = 0;
const SECTION_CLASSES: u32 = 2;
const SECTION_METHODS: u32 = 3;

/// The profile formats ART reads when the package installer hands it one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtVersion {
    /// Android 9 to 11.
    P,
    /// Android 12 onward.
    S,
}

impl BaselineProfile {
    /// The APK entries that carry this profile, as [`super::ENTRIES`] names them.
    pub(crate) fn to_entries(&self) -> Result<[(&'static str, Vec<u8>); 3]> {
        Ok([
            (PROFILE_ENTRY, self.to_asset()?),
            (METADATA_ENTRY, self.to_metadata_asset()?),
            (EMBEDDED_ENTRY, self.to_installed(ArtVersion::S)?),
        ])
    }

    /// The profile ART reads for an app installed with this as its base APK,
    /// keyed as ART names the installed DEX files.
    pub(super) fn to_installed(&self, version: ArtVersion) -> Result<Vec<u8>> {
        let key = |dex: &DexProfile| match dex.name.as_str() {
            "classes.dex" => "base.apk".to_owned(),
            name => format!("base.apk!{name}"),
        };
        match version {
            ArtVersion::P => profile_v010(&self.dex, key),
            ArtVersion::S => profile_v015(&self.dex, key),
        }
    }

    /// `baseline.prof` as AGP writes it, keyed by DEX entry name.
    fn to_asset(&self) -> Result<Vec<u8>> {
        profile_v010(&self.dex, |dex| dex.name.clone())
    }

    /// `baseline.profm` as AGP writes it.
    fn to_metadata_asset(&self) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        for (index, dex) in self.dex.iter().enumerate() {
            let metadata = metadata(dex)?;
            write_u16(&mut body, u16_len(index)?);
            write_key(&mut body, &dex.name)?;
            write_u32(&mut body, metadata.type_count);
            write_u16(&mut body, u16_len(metadata.class_defs.len())?);
            write_ascending(&mut body, &metadata.class_defs);
        }
        let mut out = header(METADATA_MAGIC, METADATA_V002);
        write_u16(&mut out, u16_len(self.dex.len())?);
        write_deflated(&mut out, &body)?;
        Ok(out)
    }
}

fn profile_v010(dex: &[DexProfile], key: impl Fn(&DexProfile) -> String) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for profile in dex {
        let hot = profile
            .methods
            .values()
            .filter(|flags| flags.contains(MethodFlags::HOT));
        let key = key(profile);
        write_u16(&mut body, u16_len(key.len())?);
        write_u16(&mut body, u16_len(profile.classes.len())?);
        write_u32(&mut body, hot.count() as u32 * 4);
        write_u32(&mut body, profile.checksum);
        write_u32(&mut body, profile.method_count);
        body.extend_from_slice(key.as_bytes());
    }
    for profile in dex {
        write_hot_methods(&mut body, profile);
        write_ascending(&mut body, &profile.classes);
        let flags = [MethodFlags::STARTUP, MethodFlags::POST_STARTUP];
        body.extend_from_slice(&method_bitmap(profile, &flags));
    }
    let mut out = header(PROFILE_MAGIC, PROFILE_V010);
    out.push(u8::try_from(dex.len()).map_err(|_| invalid(SECTION, "more than 255 DEX files"))?);
    write_deflated(&mut out, &body)?;
    Ok(out)
}

fn profile_v015(dex: &[DexProfile], key: impl Fn(&DexProfile) -> String) -> Result<Vec<u8>> {
    let mut dex_files = Vec::new();
    write_u16(&mut dex_files, u16_len(dex.len())?);
    for profile in dex {
        write_u32(&mut dex_files, profile.checksum);
        write_u32(&mut dex_files, metadata(profile)?.type_count);
        write_u32(&mut dex_files, profile.method_count);
        write_key(&mut dex_files, &key(profile))?;
    }
    let mut classes = Vec::new();
    let mut methods = Vec::new();
    for (index, profile) in dex.iter().enumerate() {
        let index = u16_len(index)?;
        write_u16(&mut classes, index);
        write_u16(&mut classes, u16_len(profile.classes.len())?);
        write_ascending(&mut classes, &profile.classes);

        let all = profile
            .methods
            .values()
            .fold(MethodFlags::empty(), |all, &flags| all | flags);
        let flags: Vec<_> = (all - MethodFlags::HOT).iter().collect();
        let mut data = Vec::new();
        write_u16(&mut data, all.bits());
        data.extend_from_slice(&method_bitmap(profile, &flags));
        write_hot_methods(&mut data, profile);
        write_u16(&mut methods, index);
        write_u32(&mut methods, data.len() as u32);
        methods.extend_from_slice(&data);
    }
    let sections = [
        (SECTION_DEX_FILES, dex_files, false),
        (SECTION_CLASSES, classes, true),
        (SECTION_METHODS, methods, true),
    ];
    let mut out = header(PROFILE_MAGIC, PROFILE_V015);
    write_u32(&mut out, sections.len() as u32);
    let mut offset = out.len() + sections.len() * 16;
    let mut contents = Vec::new();
    for (kind, data, compress) in sections {
        let stored = if compress {
            deflate(&data)?
        } else {
            data.clone()
        };
        write_u32(&mut out, kind);
        write_u32(&mut out, offset as u32);
        write_u32(&mut out, stored.len() as u32);
        write_u32(&mut out, if compress { data.len() as u32 } else { 0 });
        offset += stored.len();
        contents.extend_from_slice(&stored);
    }
    out.extend_from_slice(&contents);
    Ok(out)
}

fn metadata(profile: &DexProfile) -> Result<&DexMetadata> {
    profile.metadata.as_ref().ok_or_else(|| {
        invalid(
            SECTION,
            format!("{} has no baseline.profm entry", profile.name),
        )
    })
}

fn write_hot_methods(out: &mut Vec<u8>, profile: &DexProfile) {
    let mut last = 0;
    for (&method, flags) in &profile.methods {
        if flags.contains(MethodFlags::HOT) {
            write_u16(out, (method - last) as u16);
            write_u16(out, 0);
            last = method;
        }
    }
}

/// One run of `method_count` bits per flag, in the order given.
fn method_bitmap(profile: &DexProfile, flags: &[MethodFlags]) -> Vec<u8> {
    let count = profile.method_count as usize;
    let mut bitmap = vec![0u8; (flags.len() * count).div_ceil(8)];
    for (&method, &set) in &profile.methods {
        for (run, &flag) in flags.iter().enumerate() {
            if set.contains(flag) {
                let bit = run * count + method as usize;
                bitmap[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    bitmap
}
