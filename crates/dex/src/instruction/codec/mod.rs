// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One decoder and one encoder per instruction format; the catalogue names
//! each instruction's format and which of its fields fills each slot.

mod decode;
mod encode;
mod payload;

pub use decode::decode_instructions;
pub use encode::encode_instructions;
