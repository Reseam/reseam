// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod leb128;
pub mod mutf8;

pub mod encoded_value {
    pub use crate::write::encoded_value::*;
}
