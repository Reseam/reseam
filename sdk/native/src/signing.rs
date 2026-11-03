// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PKCS#12 backups of signing keys.

use boltffi::export;
use reseam_model::{SdkError, SigningKeyMaterial};

/// The first private key in a PKCS#12 keystore, which `password` opens. A
/// wrong password fails as `KeystoreLocked`.
#[export]
pub fn import_pkcs12(keystore: Vec<u8>, password: String) -> Result<SigningKeyMaterial, SdkError> {
    reseam_sdk::import_pkcs12(&keystore, &password).map_err(SdkError::from)
}

/// `material` as a PKCS#12 keystore that `password` opens.
#[export]
pub fn export_pkcs12(material: SigningKeyMaterial, password: String) -> Result<Vec<u8>, SdkError> {
    reseam_sdk::export_pkcs12(&material, &password).map_err(SdkError::from)
}

/// The SHA-256 of a DER certificate as uppercase hex pairs joined by colons.
#[export]
pub fn certificate_fingerprint(certificate: Vec<u8>) -> String {
    reseam_sdk::certificate_fingerprint(&certificate)
}
