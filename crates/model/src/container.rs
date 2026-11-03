// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum ContainerFormat {
    Apkm,
    Xapk,
    /// A ZIP holding the APKs at its root and nothing that describes them, as
    /// Reseam writes split output.
    Apks,
}

impl ContainerFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apkm => "apkm",
            Self::Xapk => "xapk",
            Self::Apks => "apks",
        }
    }
}
