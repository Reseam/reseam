use crate::der;
use crate::error::{Result, SignError};
use ring::rand::SystemRandom;
use ring::signature::{self, EcdsaKeyPair, KeyPair};

pub struct SigningKey {
    key_pair: EcdsaKeyPair,
    certificate_der: Vec<u8>,
}

impl SigningKey {
    /// Generate a fresh ECDSA P-256 keypair with a self-signed certificate.
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &rng,
        )
        .map_err(|e| SignError::Key { reason: format!("{e}") })?;

        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            pkcs8_doc.as_ref(),
            &rng,
        )
        .map_err(|e| SignError::Key { reason: format!("{e}") })?;

        let certificate_der = build_self_signed_cert(&key_pair, key_pair.public_key().as_ref(), &rng)?;
        Ok(Self { key_pair, certificate_der })
    }

    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let rng = SystemRandom::new();
        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            pkcs8_der,
            &rng,
        )
        .map_err(|e| SignError::Key { reason: format!("{e}") })?;
        Ok(Self { key_pair, certificate_der })
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let rng = SystemRandom::new();
        let sig = self.key_pair.sign(&rng, data)
            .map_err(|e| SignError::Crypto { reason: format!("{e}") })?;
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
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &rng,
        )
        .map_err(|e| SignError::Key { reason: format!("{e}") })?;

        let pkcs8_der = pkcs8_doc.as_ref().to_vec();
        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &pkcs8_der,
            &rng,
        )
        .map_err(|e| SignError::Key { reason: format!("{e}") })?;

        let certificate_der = build_self_signed_cert(&key_pair, key_pair.public_key().as_ref(), &rng)?;
        Ok(Self {
            signing_key: SigningKey { key_pair, certificate_der },
            pkcs8_der,
        })
    }

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
    let issuer = der::name("stitch", "stitch");
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

    let tbs_sig = key_pair.sign(rng, &tbs)
        .map_err(|e| SignError::Crypto { reason: format!("{e}") })?;

    Ok(der::sequence(&[&tbs, &sig_algo, &der::bit_string(tbs_sig.as_ref())]))
}
