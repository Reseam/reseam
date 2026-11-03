// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PublicKeyData};
use tracing::instrument;

use crate::error::{Result, crypto, invalid};
use crate::pkcs8::p256_with_public_point;

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
    /// A key without its public point takes it from the certificate. Another
    /// kind of key is `UnsupportedKey`; malformed certificates and
    /// certificates for a different key are errors.
    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let (remaining, certificate) = x509_parser::parse_x509_certificate(&certificate_der)
            .map_err(|error| invalid("certificate", error.to_string()))?;
        if !remaining.is_empty() {
            return Err(invalid("certificate", "trailing bytes after certificate"));
        }
        let pkcs8_der =
            p256_with_public_point(pkcs8_der, &certificate.public_key().subject_public_key.data)?;
        let key_pair = KeyPair::from_pkcs8_der_and_sign_algo(
            &pkcs8_der.as_slice().into(),
            &rcgen::PKCS_ECDSA_P256_SHA256,
        )
        .map_err(|source| crypto("decoding PKCS#8", source))?;
        let public_key_der = key_pair.subject_public_key_info();
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

    pub(crate) fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        rcgen::SigningKey::sign(&self.key_pair, data)
            .map_err(|source| crypto("signing payload", source))
    }

    pub fn pkcs8_der(&self) -> &[u8] {
        self.key_pair.serialized_der()
    }

    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    pub fn public_key_der(&self) -> &[u8] {
        &self.public_key_der
    }
}
