use crate::error::{PatcherError, Result};
use crate::patch::Patch;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct BundleManifest {
    bundle: BundleInfo,
    #[serde(default)]
    patches: Vec<String>,
    #[serde(default)]
    native_patches: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BundleInfo {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    target_package: Option<String>,
    #[serde(default)]
    target_versions: Vec<String>,
}

pub struct PatchBundle {
    pub name: String,
    pub author: String,
    pub description: String,
    pub target_package: Option<String>,
    pub target_versions: Vec<String>,
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

        #[cfg(feature = "lua")]
        for script in &manifest.patches {
            let script_path = dir.join(script);
            if !script_path.exists() {
                return Err(PatcherError::Bundle {
                    reason: format!("patch file not found: {}", script_path.display()),
                });
            }
            patches.push(crate::lua::load_lua_patch(&script_path)?);
        }

        #[cfg(not(feature = "lua"))]
        if !manifest.patches.is_empty() {
            return Err(PatcherError::Bundle {
                reason: "bundle contains Lua patches but the 'lua' feature is disabled".into(),
            });
        }

        #[cfg(feature = "native")]
        for lib in &manifest.native_patches {
            let lib_path = dir.join(lib);
            if !lib_path.exists() {
                return Err(PatcherError::Bundle {
                    reason: format!("native patch not found: {}", lib_path.display()),
                });
            }
            patches.push(crate::native::load_native_patch(&lib_path)?);
        }

        #[cfg(not(feature = "native"))]
        if !manifest.native_patches.is_empty() {
            return Err(PatcherError::Bundle {
                reason: "bundle contains native patches but the 'native' feature is disabled"
                    .into(),
            });
        }

        let ext_dir = dir.join("extensions");
        let mut extension_dex = Vec::new();
        if ext_dir.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(&ext_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "dex"))
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
            target_package: manifest.bundle.target_package,
            target_versions: manifest.bundle.target_versions,
            patches,
            extension_dex,
        })
    }
}
