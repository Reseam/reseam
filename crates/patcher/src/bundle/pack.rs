// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{
    bundle_error, is_payload, BundleInfo, BundleManifest, BUNDLE_FORMAT_VERSION, BUNDLE_MIMETYPE,
    ENGINE_VERSION,
};
use crate::error::Result;

/// Packs `dir/manifest.toml` and the `.jar`/`.dex` files beside it into a
/// signed bundle at `out`. Jars must carry both JVM classes and `classes.dex`
/// so the same bundle runs on the desktop JVM and on ART.
pub fn pack(dir: &Path, signing_key: &SigningKey, out: &Path) -> Result<()> {
    let manifest: BundleManifest =
        toml::from_str(&std::fs::read_to_string(dir.join("manifest.toml"))?)?;
    if manifest.bundle.format_version != BUNDLE_FORMAT_VERSION {
        return Err(bundle_error(format!(
            "unsupported format_version {} (this build packs {})",
            manifest.bundle.format_version, BUNDLE_FORMAT_VERSION
        )));
    }

    let mut payload = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !path.is_file() || !is_payload(name) {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        if name.ends_with(".jar") {
            check_universal_jar(name, &bytes)?;
        }
        payload.insert(name.to_string(), bytes);
    }
    if payload.is_empty() {
        return Err(bundle_error(format!(
            "no .jar or .dex files in {}",
            dir.display()
        )));
    }

    let manifest = BundleManifest {
        bundle: BundleInfo {
            engine: ENGINE_VERSION.to_string(),
            ..manifest.bundle
        },
        files: payload
            .iter()
            .map(|(name, bytes)| (name.clone(), hex::encode(Sha256::digest(bytes))))
            .collect(),
    };
    let manifest_bytes = toml::to_string(&manifest)
        .map_err(|e| bundle_error(format!("serialize manifest: {e}")))?
        .into_bytes();
    let signature = signing_key.sign(&manifest_bytes).to_bytes();

    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = ZipWriter::new(File::create(out)?);
    zip.start_file("mimetype", stored)?;
    zip.write_all(BUNDLE_MIMETYPE.as_bytes())?;
    zip.start_file("manifest.toml", deflated)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file("manifest.pubkey", stored)?;
    zip.write_all(&signing_key.verifying_key().to_bytes())?;
    zip.start_file("manifest.sig", stored)?;
    zip.write_all(&signature)?;
    for (name, bytes) in &payload {
        zip.start_file(name, deflated)?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    Ok(())
}

fn check_universal_jar(name: &str, bytes: &[u8]) -> Result<()> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| bundle_error(format!("{name} is not a valid jar: {e}")))?;
    let mut has_class = false;
    let mut has_dex = false;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let entry_name = entry.name();
        has_class |= entry_name.ends_with(".class") && !entry_name.starts_with("META-INF/");
        has_dex |= entry_name.starts_with("classes") && entry_name.ends_with(".dex");
    }
    if !has_class {
        return Err(bundle_error(format!("{name} is missing JVM .class files")));
    }
    if !has_dex {
        return Err(bundle_error(format!(
            "{name} is missing classes.dex; build patch jars as universal JVM/Android jars"
        )));
    }
    Ok(())
}
