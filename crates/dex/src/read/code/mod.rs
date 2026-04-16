// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Code item decoding split into focused helper modules.

pub use orchestration::read_code_item;

mod arithmetic;
mod decode;
mod format;
mod invoke;
mod memory;
mod orchestration;
mod payload;
