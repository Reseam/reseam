// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::class::read_class_skeleton_at;
use super::read_u32;
use crate::encoding::leb128::read_uleb128_with_opts;
use crate::error::{Result, invalid, require_len};
use crate::file::hidden_api::{FlagClass, FlagSource};
use crate::file::{DexBytes, DexFile, HiddenApiData};
use crate::types::header::ParseOptions;
use crate::types::hidden_api::{ClassHiddenApiFlags, HiddenApiFlags};

pub(crate) fn read_hidden_api(
    raw: DexBytes,
    offset: usize,
    dex: &DexFile,
    options: ParseOptions,
) -> Result<HiddenApiData> {
    let buf = raw.as_bytes();
    let size = read_u32(buf, offset)? as usize;
    let table_size = dex
        .classes
        .len()
        .checked_add(1)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| invalid("hidden API", "table size overflow"))?;
    if size < table_size {
        return Err(invalid(
            "hidden API",
            "item is smaller than its class offset table",
        ));
    }
    require_len(buf, offset, size, "hidden API")?;
    let end = offset + size;
    let mut classes = Vec::new();
    for i in 0..dex.classes.len() {
        let relative = read_u32(buf, offset + 4 + i * 4)? as usize;
        if relative == 0 {
            continue;
        }
        if relative > size {
            return Err(invalid(
                "hidden API",
                "class flag offset is outside item data",
            ));
        }
        let definition = dex
            .classes
            .raw_def(i)
            .expect("hidden API records are indexed before classes are materialized");
        if definition.class_data_off == 0 {
            continue;
        }
        classes.push((
            definition.header().class_type,
            FlagClass {
                data_offset: definition.class_data_off,
                flags_offset: offset + relative,
            },
        ));
    }
    let source = FlagSource {
        bytes: raw,
        end,
        options,
        classes,
    };
    for &(_, class) in &source.classes {
        read_class_flags(&source, class)?;
    }
    Ok(HiddenApiData::from_source(source))
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
        let (value, size) = read_uleb128_with_opts(
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
