// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[boltffi::data]
pub enum ContainerFormat {
    Apkm,
    Xapk,
}

impl ContainerFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apkm => "apkm",
            Self::Xapk => "xapk",
        }
    }
}
