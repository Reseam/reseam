// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{HEADER_LEN, NAME_UNITS, PackageChunk, ResPackage};
use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::chunk;
use crate::error::{Result, malformed};
use crate::resources::{RES_TABLE_TYPE_SPEC, RES_TABLE_TYPE_TYPE, ResType, TypeSpec};
use crate::string_pool::StringPool;
use reseam_storage::Bytes;
use std::ops::Range;

impl ResPackage {
    pub(in crate::resources) fn parse(
        data: &Bytes,
        chunk: Range<usize>,
        header_size: usize,
    ) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(284), "resource package")?;
        let id = read_u32_le(buf, 8, "resource package")?;
        let mut name = [0; NAME_UNITS];
        for (i, unit) in name.iter_mut().enumerate() {
            *unit = read_u16_le(buf, 12 + i * 2, "package name")?;
        }
        let type_strings_offset = read_u32_le(buf, 268, "resource package")? as usize;
        let last_public_type = read_u32_le(buf, 272, "resource package")?;
        let key_strings_offset = read_u32_le(buf, 276, "resource package")? as usize;
        let last_public_key = read_u32_le(buf, 280, "resource package")?;
        let type_id_offset = if header_size >= HEADER_LEN {
            read_u32_le(buf, 284, "resource package")?
        } else {
            0
        };

        let pool = |offset: usize, what: &'static str| -> Result<StringPool> {
            if offset == 0 {
                return Ok(StringPool::new(Vec::new(), crate::StringEncoding::Utf8));
            }
            if offset >= buf.len() {
                return Err(malformed("resource package", offset, what));
            }
            let end = chunk::chunk_end(buf, offset)?;
            StringPool::parse(data, chunk.start + offset..chunk.start + end)
        };
        let type_strings = pool(
            type_strings_offset,
            "type string pool offset is outside package",
        )?;
        let key_strings = pool(
            key_strings_offset,
            "key string pool offset is outside package",
        )?;
        let mut type_specs = Vec::new();
        let mut types = Vec::new();
        let mut chunks = Vec::new();
        let mut last = header_size;
        for sub in chunk::chunks(buf, header_size..buf.len(), "package chunk")? {
            let range = chunk.start + sub.range.start..chunk.start + sub.range.end;
            last = range.end;
            chunks.push(
                if sub.range.start == type_strings_offset && type_strings_offset != 0 {
                    PackageChunk::TypeStrings
                } else if sub.range.start == key_strings_offset && key_strings_offset != 0 {
                    PackageChunk::KeyStrings
                } else {
                    match sub.kind {
                        RES_TABLE_TYPE_SPEC => {
                            let i = type_specs.len();
                            type_specs.push(TypeSpec::parse(data, range, sub.header_size)?);
                            PackageChunk::Spec(i)
                        }
                        RES_TABLE_TYPE_TYPE => {
                            let i = types.len();
                            types.push(ResType::parse(data, range, sub.header_size)?);
                            PackageChunk::Type(i)
                        }
                        _ => PackageChunk::Raw(range),
                    }
                },
            );
        }
        if last < chunk.end {
            chunks.push(PackageChunk::Raw(last..chunk.end));
        }

        Ok(Self {
            id,
            name,
            type_strings,
            key_strings,
            last_public_type,
            last_public_key,
            type_id_offset,
            type_specs,
            types,
            data: data.clone(),
            chunk,
            header_size,
            chunks,
        })
    }
}
