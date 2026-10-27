// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_storage::{ScratchDir, copy_hashed};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{
    BUNDLE_MIMETYPE, BundleInfo, BundleKey, BundleManifest, ENGINE_VERSION, PATCH_INDEX,
    PayloadKind, bundle_error, payload_kind, read_manifest,
};
use crate::error::Result;

/// Packs `dir/manifest.toml`, adjacent `.jar`/`.dex` files, and `resources/` into a
/// signed bundle at `out`. Jars must carry both JVM classes and `classes.dex`
/// so the same bundle runs on the desktop JVM and on ART.
/// Initializes the author's patch declarations to include their static metadata
/// in the signed manifest. Packing therefore requires trusted source code.
/// `version` names the release; development builds pass `None`.
pub fn pack(dir: &Path, version: Option<&str>, key: &BundleKey, out: &Path) -> Result<()> {
    if version.is_some_and(|version| version.trim().is_empty()) {
        return Err(bundle_error("bundle version must not be empty"));
    }
    let manifest = read_manifest(&dir.join("manifest.toml"))?;
    let paths = payload_paths(dir)?;
    let mut payload = BTreeMap::new();
    let snapshot = Arc::new(ScratchDir::new("bundle-pack")?);

    for (name, path) in paths {
        let mut file = File::open(&path)
            .map_err(|error| bundle_error(format!("open payload {}: {error}", path.display())))?;
        if payload_kind(&name)? == PayloadKind::Jar {
            check_universal_jar(&name, &mut file)?;
            file.rewind()?;
        }
        let staged = snapshot.path().join(&name);
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let hash = copy_hashed(&mut file, &mut File::create(&staged)?)
            .map_err(|error| bundle_error(format!("hash payload {name}: {error}")))?;
        payload.insert(name, (staged, hash));
    }
    let jars: Vec<_> = payload
        .iter()
        .filter(|(name, _)| Path::new(name).extension().is_some_and(|ext| ext == "jar"))
        .map(|(_, (path, _))| path.clone())
        .collect();
    let bundle = BundleInfo {
        version: version.unwrap_or_default().to_owned(),
        engine: ENGINE_VERSION.to_string(),
        ..manifest.bundle
    };
    #[cfg(feature = "kotlin")]
    let patches = crate::kotlin::load_patches(
        &jars,
        &super::declarations::declarations(&jars)?,
        Arc::clone(&snapshot),
        &bundle,
    )?
    .iter()
    .map(|patch| patch.spec().clone())
    .collect();
    #[cfg(not(feature = "kotlin"))]
    let patches = {
        if !jars.is_empty() {
            return Err(bundle_error("packing patch jars requires Kotlin support"));
        }
        Vec::new()
    };
    let manifest = BundleManifest {
        bundle,
        files: payload
            .iter()
            .map(|(name, (_, hash))| (name.clone(), hex::encode(hash)))
            .collect(),
        patches,
    };
    manifest.check_patches()?;
    let manifest_bytes = toml::to_string(&manifest)
        .map_err(|e| bundle_error(format!("serialize manifest: {e}")))?
        .into_bytes();
    let signature = key.sign(&manifest_bytes);

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
    zip.write_all(&key.public_key())?;
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

fn payload_paths(dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
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
    Ok(paths)
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
