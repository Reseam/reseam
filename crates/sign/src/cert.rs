// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair};

use crate::der;
use crate::error::{internal, Result};

const NAME: &str = "reseam";
const NOT_BEFORE: &str = "240101000000Z";
const NOT_AFTER: &str = "490101000000Z";
const X509_VERSION_3: u64 = 2;
const SERIAL: u64 = 1;

pub fn self_signed(key_pair: &EcdsaKeyPair, rng: &SystemRandom) -> Result<Vec<u8>> {
    let name = der::name(NAME, NAME);
    let algorithm = der::ecdsa_sha256_algorithm();
    let tbs = der::sequence(&[
        &der::explicit_tag(0, &der::integer_u64(X509_VERSION_3)),
        &der::integer_u64(SERIAL),
        &algorithm,
        &name,
        &der::validity(NOT_BEFORE, NOT_AFTER),
        &name,
        &der::ec_subject_public_key_info(key_pair.public_key().as_ref()),
    ]);
    let signature = key_pair
        .sign(rng, &tbs)
        .map_err(|e| internal("signing certificate", e.to_string()))?;
    Ok(der::sequence(&[
        &tbs,
        &algorithm,
        &der::bit_string(signature.as_ref()),
    ]))
}
