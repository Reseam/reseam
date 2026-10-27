// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::Trust;

use crate::error::{HostError, Result};

/// The Ed25519 signing keys a host accepts bundles from. The engine ships no
/// keys of its own: the trust decision belongs to the host.
#[derive(Debug, Clone, Default)]
pub(crate) struct TrustStore {
    keys: Vec<[u8; 32]>,
}

impl TrustStore {
    /// Decodes exactly 32 bytes per key; malformed hex and lengths are errors.
    pub(crate) fn new(trust: &Trust) -> Result<Self> {
        let mut keys = trust
            .keys
            .iter()
            .map(|key| {
                let mut bytes = [0; 32];
                hex::decode_to_slice(key, &mut bytes).map_err(|source| HostError::Trust {
                    key: key.clone(),
                    source,
                })?;
                Ok(bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        keys.sort_unstable();
        keys.dedup();
        Ok(Self { keys })
    }

    pub(crate) fn contains(&self, key: &[u8; 32]) -> bool {
        self.keys.binary_search(key).is_ok()
    }
}
