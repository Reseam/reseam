// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::ops::Range;

use reseam_dex::file::DexBytes;

use super::res_type::TypePlan;
use super::{ResType, TypeSpec, RES_TABLE_PACKAGE_TYPE, RES_TABLE_TYPE_SPEC, RES_TABLE_TYPE_TYPE};
use crate::buf::{read_u16_le, read_u32_le, require_len, write_u16, write_u32};
use crate::chunk::{self, write_header};
use crate::error::{malformed, Result};
use crate::string_pool::{PoolPlan, StringPool};

const HEADER_LEN: usize = 288;
const NAME_UNITS: usize = 128;

#[derive(Debug, Clone)]
pub struct ResPackage {
    pub id: u32,
    pub name: String,
    pub type_strings: StringPool,
    pub key_strings: StringPool,
    pub last_public_type: u32,
    pub last_public_key: u32,
    pub type_id_offset: u32,
    pub type_specs: Vec<TypeSpec>,
    pub types: Vec<ResType>,
}

impl ResPackage {
    pub fn new(id: u32, name: &str, type_strings: StringPool, key_strings: StringPool) -> Self {
        Self {
            id,
            name: name.to_string(),
            type_strings,
            key_strings,
            last_public_type: 0,
            last_public_key: 0,
            type_id_offset: 0,
            type_specs: Vec::new(),
            types: Vec::new(),
        }
    }

    pub(super) fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "resource package")?;
        let id = read_u32_le(buf, 8, "resource package")?;
        let name_units = (0..NAME_UNITS)
            .map(|i| read_u16_le(buf, 12 + i * 2, "package name"))
            .take_while(|unit| !matches!(unit, Ok(0)))
            .collect::<Result<Vec<u16>>>()?;
        let name = String::from_utf16(&name_units).unwrap_or_default();
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
                return Ok(StringPool::new(Vec::new(), true));
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
        let body_start = match (key_strings_offset, type_strings_offset) {
            (0, 0) => header_size,
            (0, offset) | (offset, _) => chunk::chunk_end(buf, offset)?,
        };

        let mut type_specs = Vec::new();
        let mut types = Vec::new();
        for sub in chunk::chunks(buf, body_start..buf.len(), "package chunk")? {
            let range = chunk.start + sub.range.start..chunk.start + sub.range.end;
            match sub.kind {
                RES_TABLE_TYPE_SPEC => {
                    type_specs.push(TypeSpec::parse(data, range, sub.header_size)?)
                }
                RES_TABLE_TYPE_TYPE => types.push(ResType::parse(data, range, sub.header_size)?),
                _ => {}
            }
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
        })
    }

    /// The id of the type named `type_name`, creating an empty type when the
    /// package has none.
    pub(crate) fn ensure_type(&mut self, type_name: &str) -> Option<u8> {
        if let Some(index) = self.type_strings.find(type_name) {
            return u8::try_from(index + 1).ok();
        }
        if self.type_strings.len() >= u8::MAX as usize {
            return None;
        }
        self.type_strings.push(type_name);
        let type_id = self.type_strings.len() as u8;
        self.type_specs.push(TypeSpec::new(type_id, Vec::new()));
        self.types.push(ResType::new(type_id, Vec::new()));
        Some(type_id)
    }

    pub(super) fn plan(&self) -> Result<PackagePlan<'_>> {
        let type_strings = self.type_strings.plan();
        let key_strings = self.key_strings.plan();
        let types = self
            .types
            .iter()
            .map(ResType::plan)
            .collect::<Result<Vec<_>>>()?;
        let size = HEADER_LEN
            + type_strings.size
            + key_strings.size
            + self.type_specs.iter().map(TypeSpec::size).sum::<usize>()
            + types.iter().map(|t| t.size).sum::<usize>();
        Ok(PackagePlan {
            package: self,
            size,
            type_strings,
            key_strings,
            types,
        })
    }
}

pub(super) struct PackagePlan<'a> {
    package: &'a ResPackage,
    pub size: usize,
    type_strings: PoolPlan<'a>,
    key_strings: PoolPlan<'a>,
    types: Vec<TypePlan<'a>>,
}

impl PackagePlan<'_> {
    pub(super) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let package = self.package;
        let mut head = Vec::with_capacity(HEADER_LEN);
        write_header(
            &mut head,
            RES_TABLE_PACKAGE_TYPE,
            HEADER_LEN as u16,
            self.size,
        );
        write_u32(&mut head, package.id);
        let name_units: Vec<u16> = package.name.encode_utf16().collect();
        for i in 0..NAME_UNITS {
            write_u16(&mut head, name_units.get(i).copied().unwrap_or(0));
        }
        write_u32(&mut head, HEADER_LEN as u32);
        write_u32(&mut head, package.last_public_type);
        write_u32(&mut head, (HEADER_LEN + self.type_strings.size) as u32);
        write_u32(&mut head, package.last_public_key);
        write_u32(&mut head, package.type_id_offset);
        out.write_all(&head)?;
        self.type_strings.write(out)?;
        self.key_strings.write(out)?;
        for spec in &package.type_specs {
            spec.write(out)?;
        }
        for res_type in &self.types {
            res_type.write(out)?;
        }
        Ok(())
    }
}
