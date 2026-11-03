// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::PatchSpec;

/// A published release index: `patches.json` for a patch bundle, or
/// `manager.json` for Reseam Manager. `reseam release` writes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct ReleaseIndex {
    /// Who publishes the releases. The key is `bundle` in every index, as the
    /// patches index it was first written for named it.
    pub bundle: Publisher,
    /// Newest first.
    pub releases: Vec<PublishedRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct Publisher {
    pub name: String,
    pub author: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    #[boltffi::default(None)]
    pub homepage: Option<String>,
    /// The bundle signer in hexadecimal; absent from `manager.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    #[boltffi::default(None)]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PublishedRelease {
    pub version: String,
    /// RFC 3339.
    pub created_at: String,
    /// The release notes, in Markdown.
    pub description: String,
    pub download_url: String,
    pub prerelease: bool,
    /// The bundle's patches. Only the newest stable release and the newest
    /// prerelease keep their list, so older ones and `manager.json` have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    #[boltffi::default(None)]
    pub patches: Option<Vec<PatchSpec>>,
}

/// Which releases a user follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum ReleaseChannel {
    Stable,
    /// Prereleases and stable releases, whichever is newer.
    Prerelease,
}

impl ReleaseIndex {
    /// The newest release on `channel`.
    pub fn latest(&self, channel: ReleaseChannel) -> Option<&PublishedRelease> {
        self.releases
            .iter()
            .find(|release| channel == ReleaseChannel::Prerelease || !release.prerelease)
    }
}
