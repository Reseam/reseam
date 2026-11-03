// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PKCS#8 keys as signing takes them: P-256, with the public point in their
//! `ECPrivateKey`. Java leaves the point out, so it is filled in from the
//! certificate the key comes with.

use der::asn1::{BitStringRef, ObjectIdentifier, OctetStringRef};
use der::{Decode, Encode, Sequence};
use pkcs8::PrivateKeyInfoRef;

use crate::error::{Result, SignError, invalid};

/// `id-ecPublicKey`, the algorithm of every EC key.
const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
/// `prime256v1`, the curve APK signing uses.
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");

/// SEC 1 `ECPrivateKey`.
#[derive(Sequence)]
struct EcPrivateKey<'a> {
    version: u8,
    private_key: &'a OctetStringRef,
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    parameters: Option<ObjectIdentifier>,
    #[asn1(context_specific = "1", tag_mode = "EXPLICIT", optional = "true")]
    public_key: Option<BitStringRef<'a>>,
}

/// The P-256 key `pkcs8` with `public_point`, its uncompressed public point,
/// in its `ECPrivateKey` when it was left out. Any other kind of key is
/// `UnsupportedKey`.
pub(crate) fn p256_with_public_point(pkcs8: &[u8], public_point: &[u8]) -> Result<Vec<u8>> {
    let info = PrivateKeyInfoRef::from_der(pkcs8).map_err(|error| malformed(&error))?;
    if info.algorithm.oid != EC_PUBLIC_KEY {
        return Err(SignError::UnsupportedKey);
    }
    let curve = info
        .algorithm
        .parameters
        .map(der::AnyRef::decode_as::<ObjectIdentifier>)
        .transpose()
        .map_err(|error| malformed(&error))?;
    if curve != Some(P256) {
        return Err(SignError::UnsupportedKey);
    }
    let key =
        EcPrivateKey::from_der(info.private_key.as_bytes()).map_err(|error| malformed(&error))?;
    if key.public_key.is_some() {
        return Ok(pkcs8.to_vec());
    }
    let key = EcPrivateKey {
        public_key: Some(
            BitStringRef::from_bytes(public_point).map_err(|error| malformed(&error))?,
        ),
        ..key
    }
    .to_der()
    .map_err(|error| malformed(&error))?;
    PrivateKeyInfoRef {
        private_key: OctetStringRef::new(&key).map_err(|error| malformed(&error))?,
        ..info
    }
    .to_der()
    .map_err(|error| malformed(&error))
}

fn malformed(error: &der::Error) -> SignError {
    invalid("PKCS#8 private key", error.to_string())
}
