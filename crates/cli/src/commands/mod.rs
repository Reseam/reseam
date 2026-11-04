// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod bundle;
mod device;
mod info;
mod patch;
mod perf;
mod release;

use std::path::Path;

use anyhow::{Context, Result};
use reseam_model::Trust;

use crate::app::TrustArgs;

pub use bundle::{
    run_bundle_fetch, run_bundle_init, run_bundle_keygen, run_bundle_list, run_bundle_manifest,
    run_bundle_pack, run_bundle_pubkey, run_bundle_sign,
};
pub use device::{run_install, run_mount, run_unmount};
pub use info::run_info;
pub use patch::run_patch;
pub use perf::{run_perf, run_perf_worker};
pub use release::{run_release_files, run_release_manager, run_release_patches};

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

impl TrustArgs {
    fn trust(&self) -> Trust {
        Trust {
            keys: self.trust.clone(),
        }
    }
}
