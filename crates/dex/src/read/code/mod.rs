// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub use decode::{count_instructions, decode_instructions};
pub use orchestration::read_code_item;
pub(crate) use refs::index_operands;
pub use refs::{RawInstruction, walk_instructions};

mod arithmetic;
mod decode;
mod invoke;
mod memory;
mod orchestration;
pub(crate) mod payload;
pub(crate) mod refs;
