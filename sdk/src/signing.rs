// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::SigningKeyMaterial;
use reseam_sign::SigningKey;
use sha2::Digest;

use crate::error::Result;

/// The first private key in a PKCS#12 keystore, which `password` opens,
/// ready to sign with. Keys Java wrote without their public point get it from
/// their certificate.
pub fn import_pkcs12(keystore: &[u8], password: &str) -> Result<SigningKeyMaterial> {
    let key = SigningKey::from_pkcs12(keystore, password)?;
    Ok(SigningKeyMaterial {
        key: key.pkcs8_der().to_vec(),
        cert: key.certificate_der().to_vec(),
    })
}

/// `material` as a PKCS#12 keystore that `password` opens, the backup format
/// every Reseam app reads.
pub fn export_pkcs12(material: &SigningKeyMaterial, password: &str) -> Result<Vec<u8>> {
    Ok(SigningKey::from_pkcs8(&material.key, material.cert.clone())?.to_pkcs12(password)?)
}

/// The SHA-256 of a DER certificate as uppercase hex pairs joined by colons,
/// as `keytool` and Android print it.
pub fn certificate_fingerprint(certificate: &[u8]) -> String {
    sha2::Sha256::digest(certificate)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}
