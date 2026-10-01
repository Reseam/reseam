// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PublicKeyData};
use tracing::instrument;

use crate::error::{Result, SignError, crypto, invalid};

/// An ECDSA P-256 private key and its matching X.509 certificate.
/// Construction validates the certificate and subject public key, preserving its DER bytes.
/// Certificate trust, validity dates and issuer signatures are not APK signing requirements.
pub struct SigningKey {
    key_pair: KeyPair,
    certificate_der: Vec<u8>,
    public_key_der: Vec<u8>,
}

impl SigningKey {
    #[instrument(level = "info", skip_all)]
    pub fn generate() -> Result<Self> {
        let key_pair =
            KeyPair::generate().map_err(|source| crypto("generating signing key", source))?;
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, "reseam");
        name.push(DnType::OrganizationName, "reseam");
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|source| crypto("configuring certificate", source))?;
        params.distinguished_name = name;
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(2049, 1, 1);
        params.serial_number = Some(1_u64.into());
        let certificate_der = params
            .self_signed(&key_pair)
            .map_err(|source| crypto("issuing certificate", source))?
            .der()
            .to_vec();
        let public_key_der = key_pair.subject_public_key_info();
        Ok(Self {
            key_pair,
            certificate_der,
            public_key_der,
        })
    }

    /// Loads a PKCS#8 P-256 private key and exactly one DER X.509 certificate.
    /// Malformed certificates and certificates for a different key or curve are errors.
    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let key_pair = KeyPair::from_pkcs8_der_and_sign_algo(
            &pkcs8_der.into(),
            &rcgen::PKCS_ECDSA_P256_SHA256,
        )
        .map_err(|source| crypto("decoding PKCS#8", source))?;
        let public_key_der = key_pair.subject_public_key_info();
        let (remaining, certificate) = x509_parser::parse_x509_certificate(&certificate_der)
            .map_err(|error| invalid("certificate", error.to_string()))?;
        if !remaining.is_empty() {
            return Err(invalid("certificate", "trailing bytes after certificate"));
        }
        if certificate.public_key().raw != public_key_der {
            return Err(invalid(
                "certificate",
                "subject public key does not match private key",
            ));
        }
        Ok(Self {
            key_pair,
            certificate_der,
            public_key_der,
        })
    }

    pub fn from_files(key_path: &Path, cert_path: &Path) -> Result<Self> {
        Self::from_pkcs8(
            &crate::credentials::read(key_path)?,
            crate::credentials::read(cert_path)?,
        )
        .map_err(|source| SignError::Identity {
            key: key_path.to_owned(),
            cert: cert_path.to_owned(),
            source: Box::new(source),
        })
    }

    /// Reuses existing credentials or generates them when both paths are absent.
    /// A partial pair is an error: restore the missing file or delete the other to regenerate.
    pub fn load_or_generate(key_path: &Path, cert_path: &Path) -> Result<Self> {
        crate::credentials::load_or_generate(key_path, cert_path)
    }

    /// Publishes this pair without replacing either credential.
    /// Each file is staged in its destination directory before publication. An interruption
    /// between publications can leave a partial pair that must be restored or removed.
    /// Both parents must exist; the certificate can live in another directory/filesystem.
    pub fn save(&self, key_path: &Path, cert_path: &Path) -> Result<()> {
        crate::credentials::save(self, key_path, cert_path)
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        rcgen::SigningKey::sign(&self.key_pair, data)
            .map_err(|source| crypto("signing payload", source))
    }

    pub fn pkcs8_der(&self) -> &[u8] {
        self.key_pair.serialized_der()
    }

    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    pub fn public_key_bytes(&self) -> &[u8] {
        self.key_pair.public_key_raw()
    }

    pub fn public_key_der(&self) -> &[u8] {
        &self.public_key_der
    }
}
