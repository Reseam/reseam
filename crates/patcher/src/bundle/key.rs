// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Read, Write};
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use ring::rand::{SecureRandom, SystemRandom};

use crate::error::{BundleError, Result};

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
            .map_err(|_| BundleError::KeyGeneration)?;
        let dir = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut staged = tempfile::NamedTempFile::new_in(dir)?;
        staged.write_all(&seed)?;
        staged.persist_noclobber(path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                BundleError::KeyExists(path.to_owned())
            } else {
                BundleError::Key {
                    path: path.to_owned(),
                    source: error.error,
                }
            }
        })?;
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    pub fn read(path: &Path) -> Result<Self> {
        let invalid = || BundleError::KeyLength(path.to_owned());
        let mut file = std::fs::File::open(path).map_err(|source| BundleError::Key {
            path: path.to_owned(),
            source,
        })?;
        let mut seed = [0; 32];
        file.read_exact(&mut seed).map_err(|_| invalid())?;
        if file.read(&mut [0])? != 0 {
            return Err(invalid().into());
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
