use crate::error::{PatcherError, Result};
use crate::patch::Patch;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

#[derive(Debug, Deserialize)]
struct BundleManifest {
    bundle: BundleInfo,
}

#[derive(Debug, Deserialize)]
struct BundleInfo {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
}

pub struct PatchBundle {
    pub name: String,
    pub author: String,
    pub description: String,
    pub patches: Vec<Box<dyn Patch>>,
    pub extension_dex: Vec<PathBuf>,
}

impl PatchBundle {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let dir = path.as_ref();
        let toml_path = dir.join("bundle.toml");
        info!(
            bundle_dir = %dir.display(),
            manifest_path = %toml_path.display(),
            "loading patch bundle"
        );

        let contents = std::fs::read_to_string(&toml_path).map_err(|e| PatcherError::Bundle {
            reason: format!("failed to read {}: {e}", toml_path.display()),
        })?;

        let manifest: BundleManifest = toml::from_str(&contents)?;
        let mut patches: Vec<Box<dyn Patch>> = Vec::new();

        #[cfg(feature = "kotlin")]
        {
            let jar_files = discover_files(dir, "jar")?;
            debug!(jar_count = jar_files.len(), "discovered Kotlin patch archives");
            if !jar_files.is_empty() {
                patches.extend(crate::kotlin::load_kotlin_patches(&jar_files, dir)?);
            }
        }

        let ext_dir = dir.join("extensions");
        let mut extension_dex = Vec::new();
        if ext_dir.is_dir() {
            discover_extensions_recursive(&ext_dir, &mut extension_dex);
            extension_dex.sort();
        }

        info!(
            bundle_name = %manifest.bundle.name,
            patch_count = patches.len(),
            extension_dex_count = extension_dex.len(),
            "patch bundle loaded"
        );

        Ok(Self {
            name: manifest.bundle.name,
            author: manifest.bundle.author,
            description: manifest.bundle.description,
            patches,
            extension_dex,
        })
    }
}

fn discover_files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| PatcherError::Bundle {
            reason: format!("failed to read directory {}: {e}", dir.display()),
        })?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == extension))
        .collect();
    paths.sort();
    Ok(paths)
}

fn discover_extensions_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            discover_extensions_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "dex" || ext == "rve") {
            out.push(path);
        }
    }
}
