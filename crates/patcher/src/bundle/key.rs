// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Read, Write};
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use ring::rand::{SecureRandom, SystemRandom};

use super::bundle_error;
use crate::error::Result;

/// The Ed25519 key a bundle author signs bundles with. Its file holds exactly
/// the 32-byte private seed.
pub struct BundleKey(SigningKey);

impl BundleKey {
    /// Creates a key and writes it to `path`, which must not exist yet, in an
    /// existing directory.
    pub fn generate(path: &Path) -> Result<Self> {
        let mut seed = [0u8; 32];
        SystemRandom::new()
            .fill(&mut seed)
            .map_err(|error| bundle_error(format!("generate signing seed: {error}")))?;
        let dir = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut staged = tempfile::NamedTempFile::new_in(dir)?;
        staged.write_all(&seed)?;
        staged.persist_noclobber(path).map_err(|error| {
            bundle_error(if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("refusing to overwrite existing key at {}", path.display())
            } else {
                format!("write signing key {}: {}", path.display(), error.error)
            })
        })?;
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let invalid = || {
            bundle_error(format!(
                "signing key {} must be exactly 32 bytes",
                path.display()
            ))
        };
        let mut file = std::fs::File::open(path).map_err(|error| {
            bundle_error(format!("read signing key {}: {error}", path.display()))
        })?;
        let mut seed = [0; 32];
        file.read_exact(&mut seed).map_err(|_| invalid())?;
        if file.read(&mut [0])? != 0 {
            return Err(invalid());
        }
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub(super) fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.0.sign(message).to_bytes()
    }
}
