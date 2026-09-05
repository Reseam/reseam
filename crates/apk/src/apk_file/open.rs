// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use reseam_dex::{MultiDexContainer, ParseOptions};
use tracing::{info, instrument};

use super::{ApkComponent, ApkFile, DexOrigin};
use crate::dex;
use crate::error::Result;

impl ApkFile {
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>, opts: &ParseOptions) -> Result<Self> {
        Self::open_split(path, &[] as &[&Path], opts)
    }

    /// Opens a base APK with its split APKs; component 0 is the base.
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len()))]
    pub fn open_split(
        base: impl AsRef<Path>,
        splits: &[impl AsRef<Path>],
        opts: &ParseOptions,
    ) -> Result<Self> {
        let paths = std::iter::once((base.as_ref(), Some("base".to_string())))
            .chain(splits.iter().map(|split| (split.as_ref(), None)));
        let mut components = Vec::with_capacity(1 + splits.len());
        let mut dex = MultiDexContainer::new();
        let mut dex_origins = Vec::new();
        for (index, (path, name)) in paths.enumerate() {
            let component = ApkComponent::open(path, name, opts.lazy)?;
            for (name, file) in dex::load_dex(component.archive(), opts)? {
                dex.add_dex(file);
                dex_origins.push(DexOrigin::Existing {
                    component: index,
                    name,
                });
            }
            components.push(component);
        }
        let apk = Self {
            components,
            dex,
            dex_origins,
        };
        info!(
            package = apk.package_name().as_deref(),
            version = apk.version_name().as_deref(),
            components = apk.components.len(),
            dex_files = apk.dex.len(),
            "opened APK"
        );
        Ok(apk)
    }

    /// Parse options for patching: class data resolved on demand, no debug
    /// info, and no DEX checksums since the ZIP CRC already covered the bytes.
    pub fn patch_options() -> ParseOptions {
        ParseOptions {
            lazy: true,
            include_debug_info: false,
            include_annotations: true,
            skip_checksum: true,
            skip_signature: true,
            ..ParseOptions::default()
        }
    }
}
