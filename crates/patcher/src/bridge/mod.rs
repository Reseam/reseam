// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#![expect(
    clippy::needless_pass_by_value,
    reason = "BoltFFI exports own arguments decoded across JNI"
)]

pub(crate) mod apk;
pub(crate) mod bytecode;
pub(crate) mod changes;
pub mod dex;
mod handles;
pub(crate) mod hermes;
pub(crate) mod identity;
pub(crate) mod log_host;
pub(crate) mod options;
pub(crate) mod output;
mod phase;
pub(crate) mod run_state;
pub mod runtime;
pub(crate) mod savepoints;

pub(crate) use runtime::load_patches;
