// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub use decode::decode_instructions;
pub use item::read_code_item;

mod arithmetic;
mod decode;
mod invoke;
mod item;
mod memory;
pub(crate) mod payload;
