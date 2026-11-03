// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// A signing key as hosts store it: what `SigningIdentity::Files` points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct SigningKeyMaterial {
    /// The private key in PKCS#8 DER.
    pub key: Vec<u8>,
    /// The X.509 certificate in DER.
    pub cert: Vec<u8>,
}
