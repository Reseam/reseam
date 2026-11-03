// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PKCS#12 keystores, the one format Reseam apps back signing keys up in.

use p12_keystore::error::Error;
use p12_keystore::{
    Certificate, KeyStore, KeyStoreEntry, Pkcs12ImportPolicy, PrivateKey, PrivateKeyChain,
};
use sha1::{Digest, Sha1};

use crate::error::{Result, SignError, invalid};
use crate::key::SigningKey;

const ALIAS: &str = "reseam";

impl SigningKey {
    /// Loads the first private key and its certificate from a PKCS#12
    /// keystore that `password` opens. Keystores encrypted with AES or with
    /// the older 3DES and RC2 that Android writes both load.
    pub fn from_pkcs12(keystore: &[u8], password: &str) -> Result<Self> {
        let store = KeyStore::from_pkcs12(keystore, password, Pkcs12ImportPolicy::Relaxed)
            .map_err(|error| match error {
                Error::MacError(_) | Error::UnpadError => SignError::KeystoreLocked,
                other => invalid("PKCS#12 keystore", other.to_string()),
            })?;
        let (_, chain) = store.private_key_chain().ok_or(SignError::KeystoreEmpty)?;
        let certificate = chain.certs().first().ok_or(SignError::KeystoreEmpty)?;
        Self::from_pkcs8(chain.key().as_der(), certificate.as_der().to_vec())
    }

    /// This key and its certificate as a PKCS#12 keystore that `password`
    /// opens: AES-256 encrypted, with an HMAC-SHA256 integrity check.
    pub fn to_pkcs12(&self, password: &str) -> Result<Vec<u8>> {
        let malformed = |error: Error| invalid("PKCS#12 keystore", error.to_string());
        let certificate = self.certificate_der();
        // Tools pair a key with its certificate by this id; the certificate's
        // SHA-1 is the value Java's keytool gives it.
        let local_key_id = Sha1::digest(certificate).to_vec();
        let chain = PrivateKeyChain::new(
            local_key_id,
            PrivateKey::from_der(self.pkcs8_der()).map_err(malformed)?,
            [Certificate::from_der(certificate).map_err(malformed)?],
        );
        let mut store = KeyStore::new();
        store.add_entry(ALIAS, KeyStoreEntry::PrivateKeyChain(chain));
        store.writer(password).write().map_err(malformed)
    }
}
