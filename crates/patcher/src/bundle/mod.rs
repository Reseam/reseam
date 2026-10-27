// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `.reseam` bundles: a zip whose Ed25519-signed `manifest.toml` contains the
//! static patch catalog and every payload file's SHA-256. Whether to trust the signing
//! key is the host's decision; this module only verifies that the bundle is
//! intact and signed by the key it carries.

mod archive;
#[cfg(feature = "bridge")]
pub(crate) mod declarations;
pub(crate) mod index;
mod key;
pub(crate) mod manifest;
mod pack;
mod payload;
mod stage;

use std::path::PathBuf;

use reseam_storage::ScratchDir;

use crate::patch::{Patch, PatchSpec};

pub use archive::BundleArchive;
pub use key::BundleKey;
pub use manifest::{BundleInfo, manifest_info};
pub use pack::pack;

pub const BUNDLE_MIMETYPE: &str = "application/vnd.reseam.bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub(crate) use index::PATCH_INDEX;
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

const CONTROL_ENTRIES: [&str; 4] = [
    "mimetype",
    "manifest.toml",
    "manifest.pubkey",
    "manifest.sig",
];

/// A loaded bundle: its signed catalog and the code of the patches a run asked
/// for. The payload lives in a scratch directory for as long as the bundle
/// does, which is what keeps the patches' extension DEX readable.
pub struct PatchBundle {
    info: BundleInfo,
    public_key: [u8; 32],
    catalog: Vec<PatchSpec>,
    patches: Vec<Patch>,
    extension_dex: Vec<PathBuf>,
    _extracted: std::sync::Arc<ScratchDir>,
}

impl PatchBundle {
    pub fn info(&self) -> &BundleInfo {
        &self.info
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// Every patch the bundle declares, in catalog order.
    pub fn catalog(&self) -> &[PatchSpec] {
        &self.catalog
    }

    /// The loaded patches, a subset of the catalog.
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Takes the patches. Each retains its class loader and bundle files until
    /// it is dropped, including resources and classes loaded during execution.
    pub fn into_patches(self) -> Vec<Patch> {
        self.patches
    }

    /// Extension files remain immutable and readable for the bundle's lifetime.
    pub fn extension_dex(&self) -> &[PathBuf] {
        &self.extension_dex
    }
}

#[cfg(test)]
mod tests;
