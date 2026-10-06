// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{HostError, Result};

/// The Ed25519 signing keys a host accepts bundles from. The engine ships no
/// keys of its own: the trust decision belongs to the host.
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    keys: Vec<[u8; 32]>,
}

impl TrustStore {
    pub fn new(keys: impl IntoIterator<Item = [u8; 32]>) -> Self {
        let mut keys: Vec<_> = keys.into_iter().collect();
        keys.sort_unstable();
        keys.dedup();
        Self { keys }
    }

    /// Decodes exactly 32 bytes per key; malformed hex and lengths are errors.
    pub fn from_hex(keys: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self> {
        keys.into_iter()
            .map(|key| {
                let key = key.as_ref();
                let mut bytes = [0; 32];
                hex::decode_to_slice(key, &mut bytes).map_err(|source| HostError::Trust {
                    key: key.to_owned(),
                    source,
                })?;
                Ok(bytes)
            })
            .collect::<Result<Vec<_>>>()
            .map(Self::new)
    }

    pub fn contains(&self, key: &[u8; 32]) -> bool {
        self.keys.binary_search(key).is_ok()
    }
}

impl From<&TrustStore> for reseam_model::Trust {
    fn from(store: &TrustStore) -> Self {
        Self {
            keys: store.keys.iter().map(hex::encode).collect(),
        }
    }
}
