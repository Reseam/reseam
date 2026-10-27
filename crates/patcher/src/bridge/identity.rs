// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::{data, export};

use crate::bridge::phase::with_running_bundle;

/// The bundle whose patch is running and the engine running it.
#[data]
#[derive(Debug, Clone)]
pub struct RunningBundleInfo {
    pub name: String,
    /// Empty for a development build.
    pub version: String,
    /// Empty when the manifest names none.
    pub license: String,
    pub engine: String,
}

#[export]
pub fn running_bundle() -> Result<RunningBundleInfo, String> {
    with_running_bundle(|bundle| RunningBundleInfo {
        name: bundle.name.clone(),
        version: bundle.version.clone(),
        license: bundle.license.clone(),
        engine: crate::bundle::ENGINE_VERSION.to_owned(),
    })
}
