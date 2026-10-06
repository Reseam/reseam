// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod leb128;
pub mod mutf8;

pub mod encoded_value {
    pub use crate::write::encoded_value::*;
}
