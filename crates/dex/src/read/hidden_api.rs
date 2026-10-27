// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::class::read_class_skeleton_at;
use crate::DexBytes;
use crate::encoding::leb128::read_uleb128;
use crate::error::Result;
use crate::model::TypeIdx;
use crate::model::hidden_api::{ClassHiddenApiFlags, HiddenApiFlags};
use crate::options::ParseOptions;

/// Where one class's member flags start, and the class data listing its members.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlagClass {
    pub data_offset: u32,
    pub flags_offset: usize,
}

/// A `hiddenapi_class_data_item` left in the file, its classes sorted by type.
#[derive(Debug, Clone)]
pub(crate) struct FlagSource {
    pub bytes: DexBytes,
    pub end: usize,
    pub options: ParseOptions,
    pub classes: Vec<(TypeIdx, FlagClass)>,
}

pub(crate) fn read_class_flags(
    source: &FlagSource,
    class: FlagClass,
) -> Result<ClassHiddenApiFlags> {
    let members = read_class_skeleton_at(
        source.bytes.as_bytes(),
        class.data_offset as usize,
        source.options,
    )?;
    let mut position = class.flags_offset;
    let mut next_flag = || -> Result<HiddenApiFlags> {
        let (value, size) = read_uleb128(
            &source.bytes.as_bytes()[..source.end],
            position,
            source.options,
        )?;
        position += size;
        Ok(HiddenApiFlags::from_bits(value))
    };
    let field_flags = members
        .static_fields
        .iter()
        .chain(&members.instance_fields)
        .map(|field| Ok((field.field, next_flag()?)))
        .collect::<Result<_>>()?;
    let method_flags = members
        .direct_methods
        .iter()
        .chain(&members.virtual_methods)
        .map(|method| Ok((method.method, next_flag()?)))
        .collect::<Result<_>>()?;
    Ok(ClassHiddenApiFlags {
        field_flags,
        method_flags,
    })
}
