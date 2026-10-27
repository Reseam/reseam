// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

macro_rules! dex {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/",
            $name,
            ".dex"
        ))
        .as_slice()
    };
}

pub(crate) use dex;

pub mod apks;
pub mod bundles;
pub mod code;
