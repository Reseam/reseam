use crate::error::{PatcherError, Result};
use crate::patch::Patch;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

        let contents = std::fs::read_to_string(&toml_path).map_err(|e| PatcherError::Bundle {
            reason: format!("failed to read {}: {e}", toml_path.display()),
        })?;

        let manifest: BundleManifest = toml::from_str(&contents)?;
        let mut patches: Vec<Box<dyn Patch>> = Vec::new();

        #[cfg(feature = "kotlin")]
        {
            let jar_files = discover_files(dir, "jar");
            if !jar_files.is_empty() {
                patches.extend(crate::kotlin::load_kotlin_patches(&jar_files, dir)?);
            }
        }

        let ext_dir = dir.join("extensions");
        let mut extension_dex = Vec::new();
        if ext_dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&ext_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "dex"))
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                extension_dex.push(entry.path());
            }
        }

        Ok(Self {
            name: manifest.bundle.name,
            author: manifest.bundle.author,
            description: manifest.bundle.description,
            patches,
            extension_dex,
        })
    }
}

fn discover_files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == extension))
        .collect();
    paths.sort();
    paths
}

