// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{unsupported, Result};
use crate::keystore::SigningKey;
use tracing::instrument;

#[instrument(level = "info", skip(apk, key), fields(apk_size = apk.len()))]
pub fn sign(apk: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    sign_with_sdk_range(apk, key, 24, u32::MAX)
}

#[instrument(level = "info", skip_all, fields(min_sdk, max_sdk))]
pub fn sign_with_sdk_range(
    _apk: &[u8],
    _key: &SigningKey,
    _min_sdk: u32,
    _max_sdk: u32,
) -> Result<Vec<u8>> {
    Err(unsupported(
        "apk signature scheme v3",
        "v3 signing is not currently implemented correctly; use v2 signing instead",
    ))
}
