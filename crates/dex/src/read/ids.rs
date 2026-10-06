// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::Result;
use crate::types::TypeList;

pub fn read_type_list(buf: &[u8], off: u32) -> Result<TypeList> {
    let base = off as usize;
    crate::file::validate_type_list(buf, base)?;
    Ok(crate::file::read_type_list(buf, base))
}
