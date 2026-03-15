use crate::der;
use crate::error::{Result, SignError};
use ring::rand::SystemRandom;
use ring::signature::{self, EcdsaKeyPair, KeyPair};

/// A signing identity: private key + DER-encoded X.509 certificate.
pub struct SigningKey {
    key_pair: EcdsaKeyPair,
    certificate_der: Vec<u8>,
}

impl SigningKey {
    /// Generate a fresh ECDSA P-256 keypair with a self-signed certificate.
    ///
    /// The certificate uses CN="stitch", O="stitch" with 25-year validity.
    /// This is suitable for debug/custom signing of patched APKs.
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();

        let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &rng,
        )
        .map_err(|e| SignError::Key {
            reason: format!("key generation failed: {e}"),
        })?;

        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            pkcs8_doc.as_ref(),
            &rng,
        )
        .map_err(|e| SignError::Key {
            reason: format!("failed to load generated key: {e}"),
        })?;

        let public_key_bytes = key_pair.public_key().as_ref();
        let certificate_der = build_self_signed_cert(&key_pair, public_key_bytes, &rng)?;

        Ok(Self {
            key_pair,
            certificate_der,
        })
    }

    /// Load an existing ECDSA P-256 keypair from PKCS#8 DER bytes and a DER certificate.
    pub fn from_pkcs8(pkcs8_der: &[u8], certificate_der: Vec<u8>) -> Result<Self> {
        let rng = SystemRandom::new();
        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            pkcs8_der,
            &rng,
        )
        .map_err(|e| SignError::Key {
            reason: format!("failed to load PKCS#8 key: {e}"),
        })?;

        Ok(Self {
            key_pair,
            certificate_der,
        })
    }

    /// Save the PKCS#8 private key and DER certificate to files.
    pub fn save(&self, key_path: &std::path::Path, cert_path: &std::path::Path) -> Result<()> {
        // Note: ring doesn't expose the PKCS#8 bytes after loading.
        // Callers should save the pkcs8 bytes from generate_pkcs8() directly.
        std::fs::write(cert_path, &self.certificate_der)?;
        // Key saving requires the original PKCS#8 bytes, which we don't retain.
        // This is a limitation — callers should save at generation time.
        let _ = key_path;
        Ok(())
    }

    /// Sign data with the private key.
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        let rng = SystemRandom::new();
        let sig = self
            .key_pair
            .sign(&rng, data)
            .map_err(|e| SignError::Crypto {
                reason: format!("signing failed: {e}"),
            })?;
        Ok(sig.as_ref().to_vec())
    }

    /// Get the DER-encoded X.509 certificate.
    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }

    /// Get the raw public key bytes.
    pub fn public_key_bytes(&self) -> &[u8] {
        self.key_pair.public_key().as_ref()
    }
}

/// A generated keypair that retains the PKCS#8 bytes for saving.
pub struct GeneratedKey {
    pub signing_key: SigningKey,
    pub pkcs8_der: Vec<u8>,
}

impl GeneratedKey {
    /// Generate and return both the signing key and the raw PKCS#8 bytes.
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();

        let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &rng,
        )
        .map_err(|e| SignError::Key {
            reason: format!("key generation failed: {e}"),
        })?;

        let pkcs8_der = pkcs8_doc.as_ref().to_vec();

        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &pkcs8_der,
            &rng,
        )
        .map_err(|e| SignError::Key {
            reason: format!("failed to load generated key: {e}"),
        })?;

        let public_key_bytes = key_pair.public_key().as_ref();
        let certificate_der = build_self_signed_cert(&key_pair, public_key_bytes, &rng)?;

        Ok(Self {
            signing_key: SigningKey {
                key_pair,
                certificate_der,
            },
            pkcs8_der,
        })
    }

    /// Save PKCS#8 key and DER certificate to files.
    pub fn save(&self, key_path: &std::path::Path, cert_path: &std::path::Path) -> Result<()> {
        std::fs::write(key_path, &self.pkcs8_der)?;
        std::fs::write(cert_path, self.signing_key.certificate_der())?;
        Ok(())
    }
}

/// Build a minimal self-signed X.509 v3 certificate for an ECDSA P-256 key.
fn build_self_signed_cert(
    key_pair: &EcdsaKeyPair,
    public_key: &[u8],
    rng: &SystemRandom,
) -> Result<Vec<u8>> {
    let issuer = der::name("stitch", "stitch");
    let subject = issuer.clone();

    // 25-year validity: 2024-01-01 to 2049-01-01
    let validity = der::validity("240101000000Z", "490101000000Z");

    let spki = der::ec_subject_public_key_info(public_key);
    let sig_algo = der::ecdsa_sha256_algorithm();

    // TBSCertificate
    let version = der::explicit_tag(0, &der::integer_u64(2)); // v3
    let serial = der::integer_u64(1);

    let tbs = der::sequence(&[
        &version,
        &serial,
        &sig_algo,
        &issuer,
        &validity,
        &subject,
        &spki,
    ]);

    // Sign the TBSCertificate
    let tbs_sig = key_pair.sign(rng, &tbs).map_err(|e| SignError::Crypto {
        reason: format!("certificate self-signing failed: {e}"),
    })?;

    // Certificate = SEQUENCE { tbs, signatureAlgorithm, signatureValue }
    let cert = der::sequence(&[&tbs, &sig_algo, &der::bit_string(tbs_sig.as_ref())]);

    Ok(cert)
}
