// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::der;
use crate::error::{internal, invalid, Result};
use ring::rand::SystemRandom;
use ring::signature::{self, EcdsaKeyPair, KeyPair};
use std::path::Path;
use tracing::{debug, instrument};

pub struct SigningKey {
    key_pair: EcdsaKeyPair,
    certificate_der: Vec<u8>,
}

impl SigningKey {
    /// Generate a fresh ECDSA P-256 keypair with a self-signed certificate.
    #[instrument(level = "info", skip_all)]
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_doc =
            EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
                .map_err(|e| invalid("signing key", format!("key generation failed: {e}")))?;

        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            pkcs8_doc.as_ref(),
            &rng,
        )
        .map_err(|e| invalid("signing key", format!("pkcs8 decoding failed: {e}")))?;

        let certificate_der =
            build_self_signed_cert(&key_pair, key_pair.public_key().as_ref(), &rng)?;
        Ok(Self {
            key_pair,
            certificate_der,
        })
    }

    #[instrument(level = "debug", skip(pkcs8_der, certificate_der), fields(pkcs8_len = pkcs8_der.len(), cert_len = certificate_der.len()))]
    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let rng = SystemRandom::new();
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_der, &rng)
                .map_err(|e| invalid("signing key", format!("pkcs8 decoding failed: {e}")))?;
        Ok(Self {
            key_pair,
            certificate_der,
        })
    }

    #[instrument(level = "info", skip_all, fields(key_path = %key_path.display(), cert_path = %cert_path.display()))]
    pub fn from_files(key_path: &Path, cert_path: &Path) -> Result<Self> {
        let pkcs8_der = std::fs::read(key_path)?;
        let certificate_der = std::fs::read(cert_path)?;
        Self::from_pkcs8(&pkcs8_der, certificate_der)
    }

    #[instrument(level = "debug", skip(self, data), fields(payload_len = data.len()))]
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let rng = SystemRandom::new();
        let sig = self
            .key_pair
            .sign(&rng, data)
            .map_err(|e| internal("signing payload", format!("crypto signing failed: {e}")))?;
        debug!(signature_len = sig.as_ref().len(), "payload signed");
        Ok(sig.as_ref().to_vec())
    }

    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    pub fn public_key_bytes(&self) -> &[u8] {
        self.key_pair.public_key().as_ref()
    }
}

/// Retains the PKCS#8 bytes so they can be saved to disk.
pub struct GeneratedKey {
    pub signing_key: SigningKey,
    pub pkcs8_der: Vec<u8>,
}

impl GeneratedKey {
    #[instrument(level = "info", skip_all)]
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_doc =
            EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
                .map_err(|e| invalid("signing key", format!("key generation failed: {e}")))?;
        let pkcs8_der = pkcs8_doc.as_ref().to_vec();
        let signing_key = SigningKey::from_pkcs8(&pkcs8_der, {
            let key_pair = EcdsaKeyPair::from_pkcs8(
                &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                &pkcs8_der,
                &rng,
            )
            .map_err(|e| invalid("signing key", format!("pkcs8 decoding failed: {e}")))?;
            build_self_signed_cert(&key_pair, key_pair.public_key().as_ref(), &rng)?
        })?;
        Ok(Self {
            signing_key,
            pkcs8_der,
        })
    }

    #[instrument(level = "info", skip(self), fields(key_path = %key_path.display(), cert_path = %cert_path.display()))]
    pub fn save(&self, key_path: &std::path::Path, cert_path: &std::path::Path) -> Result<()> {
        std::fs::write(key_path, &self.pkcs8_der)?;
        std::fs::write(cert_path, self.signing_key.certificate_der())?;
        Ok(())
    }
}

fn build_self_signed_cert(
    key_pair: &EcdsaKeyPair,
    public_key: &[u8],
    rng: &SystemRandom,
) -> Result<Vec<u8>> {
    let issuer = der::name("reseam", "reseam");
    let validity = der::validity("240101000000Z", "490101000000Z");
    let spki = der::ec_subject_public_key_info(public_key);
    let sig_algo = der::ecdsa_sha256_algorithm();

    let tbs = der::sequence(&[
        &der::explicit_tag(0, &der::integer_u64(2)), // v3
        &der::integer_u64(1),
        &sig_algo,
        &issuer,
        &validity,
        &issuer, // subject = issuer (self-signed)
        &spki,
    ]);

    let tbs_sig = key_pair.sign(rng, &tbs).map_err(|e| {
        internal(
            "building self-signed certificate",
            format!("crypto signing failed: {e}"),
        )
    })?;

    Ok(der::sequence(&[
        &tbs,
        &sig_algo,
        &der::bit_string(tbs_sig.as_ref()),
    ]))
}
