// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod certificates;
mod credentials;
mod error;
mod key;
pub mod signing_block;
pub mod v2;

pub use certificates::signer_certificates;
pub use error::{Result, SignError};
pub use key::SigningKey;
