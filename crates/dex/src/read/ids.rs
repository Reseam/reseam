// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::header::{u16_at, u32_at};
use crate::error::Result;
use crate::types::{TypeIdx, TypeList};

pub fn read_type_list(buf: &[u8], off: u32) -> Result<TypeList> {
    let base = off as usize;
    let size = u32_at(buf, base)? as usize;
    let mut list = TypeList::with_capacity(size);
    for i in 0..size {
        list.push(TypeIdx(u16_at(buf, base + 4 + i * 2)? as u32));
    }
    Ok(list)
}
