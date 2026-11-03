// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_sign::{SignError, SigningKey};

const PASSWORD: &str = "reseam-test";

/// PKCS#12 keystores `keytool` wrote, each holding one P-256 key whose PKCS#8
/// leaves out the public point, as Java writes keys.
const KEYSTORES: [(&str, &[u8]); 2] = [
    ("AES", include_bytes!("keystores/pkcs12-aes.p12")),
    (
        "3DES and RC2",
        include_bytes!("keystores/pkcs12-legacy.p12"),
    ),
];

#[test]
fn keys_load_from_pkcs12_however_it_is_encrypted_and_back_up_again() {
    for (encryption, keystore) in KEYSTORES {
        let key = SigningKey::from_pkcs12(keystore, PASSWORD)
            .unwrap_or_else(|error| panic!("{encryption}: {error}"));
        let backup = key.to_pkcs12("backup").unwrap();
        let restored = SigningKey::from_pkcs12(&backup, "backup").unwrap();
        assert_eq!(restored.pkcs8_der(), key.pkcs8_der(), "{encryption}");
        assert_eq!(
            restored.certificate_der(),
            key.certificate_der(),
            "{encryption}"
        );
    }
}

#[test]
fn a_wrong_password_reads_as_a_locked_keystore() {
    for (encryption, keystore) in KEYSTORES {
        assert!(
            matches!(
                SigningKey::from_pkcs12(keystore, "wrong"),
                Err(SignError::KeystoreLocked)
            ),
            "{encryption}"
        );
    }
}

#[test]
fn a_key_signing_cannot_use_reads_as_unsupported() {
    let keystore = include_bytes!("keystores/pkcs12-rsa.p12");
    assert!(matches!(
        SigningKey::from_pkcs12(keystore, PASSWORD),
        Err(SignError::UnsupportedKey)
    ));
}
