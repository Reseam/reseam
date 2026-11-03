// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod block;
pub mod certificates;
mod credentials;
mod error;
mod key;
mod pkcs12;
mod pkcs8;
mod preserved;
pub mod v2;
mod zip_layout;

pub use certificates::signer_certificates;
pub use error::{Result, SignError};
pub use key::SigningKey;
pub use preserved::PreservedPairs;
