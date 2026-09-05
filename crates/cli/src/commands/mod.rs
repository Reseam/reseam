// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod bundle;
mod info;
mod patch;
mod perf;
mod publish;

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use reseam_sdk::TrustStore;

use crate::app::TrustArgs;

pub use bundle::{run_bundle_keygen, run_bundle_list, run_bundle_pack};
pub use info::run_info;
pub use patch::run_patch;
pub use perf::run_perf;
pub use publish::{run_publish_manager, run_publish_patches};

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
    fn store(&self) -> Result<TrustStore> {
        TrustStore::from_hex(&self.trust).map_err(|reason| anyhow!("invalid --trust: {reason}"))
    }
}
