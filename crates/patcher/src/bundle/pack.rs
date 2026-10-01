// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{
    BUNDLE_MIMETYPE, BundleInfo, BundleManifest, ENGINE_VERSION, PATCH_INDEX, PayloadKind,
    bundle_error, payload_kind, read_manifest,
};
use crate::error::Result;

/// Packs `dir/manifest.toml`, adjacent `.jar`/`.dex` files, and `resources/` into a
/// signed bundle at `out`. Jars must carry both JVM classes and `classes.dex`
/// so the same bundle runs on the desktop JVM and on ART.
pub fn pack(dir: &Path, signing_key: &SigningKey, out: &Path) -> Result<()> {
    let manifest = read_manifest(&dir.join("manifest.toml"))?;
    let mut paths = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().into_string().map_err(|name| {
            bundle_error(format!(
                "non-UTF-8 payload name: {}",
                name.to_string_lossy()
            ))
        })?;
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("jar" | "dex")
        ) {
            payload_kind(&name)?;
            if !entry.file_type()?.is_file() {
                return Err(bundle_error(format!(
                    "payload {} must be a regular file",
                    path.display()
                )));
            }
            paths.insert(name, path);
        } else if name == "resources" {
            resource_paths(&path, "resources", &mut paths)?;
        }
    }
    if paths.is_empty() {
        return Err(bundle_error(format!(
            "no payload files in {}",
            dir.display()
        )));
    }
    let mut payload = BTreeMap::new();

    for (name, path) in paths {
        let mut file = File::open(&path)
            .map_err(|error| bundle_error(format!("open payload {}: {error}", path.display())))?;
        if payload_kind(&name)? == PayloadKind::Jar {
            check_universal_jar(&name, &mut file)?;
            file.rewind()?;
        }
        let hash = copy_hashed(&mut file, &mut std::io::sink())
            .map_err(|error| bundle_error(format!("hash payload {name}: {error}")))?;
        payload.insert(name, (path, hash));
    }
    let manifest = BundleManifest {
        bundle: BundleInfo {
            engine: ENGINE_VERSION.to_string(),
            ..manifest.bundle
        },
        files: payload
            .iter()
            .map(|(name, (_, hash))| (name.clone(), hex::encode(hash)))
            .collect(),
    };
    let manifest_bytes = toml::to_string(&manifest)
        .map_err(|e| bundle_error(format!("serialize manifest: {e}")))?
        .into_bytes();
    let signature = signing_key.sign(&manifest_bytes).to_bytes();

    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temporary.as_file_mut());
    zip.start_file("mimetype", stored)?;
    zip.write_all(BUNDLE_MIMETYPE.as_bytes())?;
    zip.start_file("manifest.toml", deflated)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file("manifest.pubkey", stored)?;
    zip.write_all(&signing_key.verifying_key().to_bytes())?;
    zip.start_file("manifest.sig", stored)?;
    zip.write_all(&signature)?;
    for (name, (path, expected)) in payload {
        let mut file = File::open(&path)
            .map_err(|error| bundle_error(format!("open payload {}: {error}", path.display())))?;
        zip.start_file(&name, deflated)?;
        if copy_hashed(&mut file, &mut zip)
            .map_err(|error| bundle_error(format!("pack payload {name}: {error}")))?
            != expected
        {
            return Err(bundle_error(format!(
                "payload {name} changed while packing"
            )));
        }
    }
    zip.finish()?;
    temporary.as_file().sync_all()?;
    temporary.persist(out).map_err(|error| error.error)?;
    Ok(())
}

fn check_universal_jar(name: &str, file: &mut File) -> Result<()> {
    let mut archive = ZipArchive::new(file)
        .map_err(|e| bundle_error(format!("{name} is not a valid jar: {e}")))?;
    let mut has_class = false;
    let mut has_dex = false;
    let mut has_index = false;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let entry_name = entry.name();
        has_class |= Path::new(entry_name)
            .extension()
            .is_some_and(|extension| extension == "class")
            && !entry_name.starts_with("META-INF/");
        has_dex |= entry_name == "classes.dex";
        has_index |= entry_name == PATCH_INDEX;
    }
    if !has_class {
        return Err(bundle_error(format!("{name} is missing JVM .class files")));
    }
    if !has_dex {
        return Err(bundle_error(format!(
            "{name} is missing classes.dex; build patch jars as universal JVM/Android jars"
        )));
    }
    if !has_index {
        return Err(bundle_error(format!(
            "{name} is missing its patch declaration index; rebuild it with the Reseam Gradle plugin"
        )));
    }
    Ok(())
}

fn resource_paths(dir: &Path, prefix: &str, paths: &mut BTreeMap<String, PathBuf>) -> Result<()> {
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(bundle_error(format!(
            "{} must be a directory",
            dir.display()
        )));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|name| {
            bundle_error(format!(
                "non-UTF-8 resource name: {}",
                name.to_string_lossy()
            ))
        })?;
        let name = format!("{prefix}/{name}");
        payload_kind(&name)?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            resource_paths(&entry.path(), &name, paths)?;
        } else if kind.is_file() {
            paths.insert(name, entry.path());
        } else {
            return Err(bundle_error(format!(
                "resource {name} must be a regular file"
            )));
        }
    }
    Ok(())
}

pub(crate) fn copy_hashed(reader: &mut impl Read, writer: &mut impl Write) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let size = reader.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        writer.write_all(&buffer[..size])?;
        hash.update(&buffer[..size]);
    }
    Ok(hash.finalize().into())
}
