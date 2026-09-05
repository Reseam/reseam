// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Deserialize;

/// The Ed25519 signing keys a host accepts bundles from. The engine ships no
/// keys of its own: the trust decision belongs to the host. Deserializes from
/// `{"keys": ["<hex>", ...]}`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(try_from = "TrustSpec")]
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

    pub fn from_hex<S: AsRef<str>>(keys: impl IntoIterator<Item = S>) -> Result<Self, String> {
        keys.into_iter()
            .map(|hex_key| {
                let hex_key = hex_key.as_ref();
                let bytes = hex::decode(hex_key)
                    .map_err(|error| format!("invalid trusted key `{hex_key}`: {error}"))?;
                bytes.as_slice().try_into().map_err(|_| {
                    format!(
                        "trusted key `{hex_key}` has {} bytes; expected 32",
                        bytes.len()
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::new)
    }

    pub fn contains(&self, key: &[u8; 32]) -> bool {
        self.keys.binary_search(key).is_ok()
    }
}

#[derive(Deserialize)]
struct TrustSpec {
    #[serde(default)]
    keys: Vec<String>,
}

impl TryFrom<TrustSpec> for TrustStore {
    type Error = String;

    fn try_from(spec: TrustSpec) -> Result<Self, String> {
        Self::from_hex(&spec.keys)
    }
}
