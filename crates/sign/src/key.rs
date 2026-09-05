// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};
use tracing::instrument;

use crate::cert;
use crate::error::{internal, invalid, Result};

pub struct SigningKey {
    key_pair: EcdsaKeyPair,
    certificate_der: Vec<u8>,
    rng: SystemRandom,
}

impl SigningKey {
    pub fn generate() -> Result<Self> {
        Ok(GeneratedKey::generate()?.signing_key)
    }

    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let rng = SystemRandom::new();
        let key_pair = decode_pkcs8(pkcs8_der, &rng)?;
        Ok(Self {
            key_pair,
            certificate_der,
            rng,
        })
    }

    pub fn from_files(key_path: &Path, cert_path: &Path) -> Result<Self> {
        Self::from_pkcs8(&std::fs::read(key_path)?, std::fs::read(cert_path)?)
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let signature = self
            .key_pair
            .sign(&self.rng, data)
            .map_err(|e| internal("signing payload", e.to_string()))?;
        Ok(signature.as_ref().to_vec())
    }

    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    pub fn public_key_bytes(&self) -> &[u8] {
        self.key_pair.public_key().as_ref()
    }
}

/// A freshly generated key that still has its PKCS#8 bytes, so it can be saved.
pub struct GeneratedKey {
    pub signing_key: SigningKey,
    pub pkcs8_der: Vec<u8>,
}

impl GeneratedKey {
    #[instrument(level = "info", skip_all)]
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_der = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
            .map_err(|e| invalid("signing key", format!("key generation failed: {e}")))?
            .as_ref()
            .to_vec();
        let key_pair = decode_pkcs8(&pkcs8_der, &rng)?;
        let certificate_der = cert::self_signed(&key_pair, &rng)?;
        Ok(Self {
            signing_key: SigningKey {
                key_pair,
                certificate_der,
                rng,
            },
            pkcs8_der,
        })
    }

    pub fn save(&self, key_path: &Path, cert_path: &Path) -> Result<()> {
        std::fs::write(key_path, &self.pkcs8_der)?;
        std::fs::write(cert_path, self.signing_key.certificate_der())?;
        Ok(())
    }
}

fn decode_pkcs8(pkcs8_der: &[u8], rng: &SystemRandom) -> Result<EcdsaKeyPair> {
    EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_der, rng)
        .map_err(|e| invalid("signing key", format!("pkcs8 decoding failed: {e}")))
}
