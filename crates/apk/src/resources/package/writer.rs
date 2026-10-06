// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{HEADER_LEN, PackageChunk, ResPackage};
use crate::buf::{write_u16, write_u32};
use crate::chunk::write_header;
use crate::error::Result;
use crate::resources::res_type::TypePlan;
use crate::resources::{RES_TABLE_PACKAGE_TYPE, ResType};
use crate::string_pool::PoolPlan;
use std::io::Write;

impl ResPackage {
    pub(in crate::resources) fn plan(&self) -> Result<PackagePlan<'_>> {
        let type_strings = self.type_strings.plan()?;
        let key_strings = self.key_strings.plan()?;
        let types = self
            .types
            .iter()
            .map(ResType::plan)
            .collect::<Result<Vec<_>>>()?;
        let mut chunks = self.chunks.clone();
        if !chunks
            .iter()
            .any(|chunk| matches!(chunk, PackageChunk::TypeStrings))
            && !self.type_strings.is_empty()
        {
            chunks.insert(0, PackageChunk::TypeStrings);
        }
        if !chunks
            .iter()
            .any(|chunk| matches!(chunk, PackageChunk::KeyStrings))
            && !self.key_strings.is_empty()
        {
            chunks.insert(
                usize::from(matches!(chunks.first(), Some(PackageChunk::TypeStrings))),
                PackageChunk::KeyStrings,
            );
        }
        let size = self.header_size
            + chunks
                .iter()
                .map(|chunk| match chunk {
                    PackageChunk::TypeStrings => type_strings.size,
                    PackageChunk::KeyStrings => key_strings.size,
                    PackageChunk::Spec(i) => self.type_specs[*i].size(),
                    PackageChunk::Type(i) => types[*i].size,
                    PackageChunk::Raw(range) => range.len(),
                })
                .sum::<usize>();
        u32::try_from(size)
            .map_err(|_| crate::error::invalid("resource package", "package exceeds 4 GiB"))?;
        Ok(PackagePlan {
            package: self,
            size,
            type_strings,
            key_strings,
            types,
            chunks,
        })
    }
}

pub(in crate::resources) struct PackagePlan<'a> {
    package: &'a ResPackage,
    pub(in crate::resources) size: usize,
    type_strings: PoolPlan<'a>,
    key_strings: PoolPlan<'a>,
    types: Vec<TypePlan<'a>>,
    chunks: Vec<PackageChunk>,
}

impl PackagePlan<'_> {
    pub(in crate::resources) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let package = self.package;
        let mut head = if package.chunk.is_empty() {
            let mut head = Vec::with_capacity(HEADER_LEN);
            write_header(
                &mut head,
                RES_TABLE_PACKAGE_TYPE,
                HEADER_LEN as u16,
                self.size,
            );
            write_u32(&mut head, package.id);
            for &unit in &package.name {
                write_u16(&mut head, unit);
            }
            write_u32(&mut head, 0);
            write_u32(&mut head, package.last_public_type);
            write_u32(&mut head, 0);
            write_u32(&mut head, package.last_public_key);
            write_u32(&mut head, package.type_id_offset);
            head
        } else {
            package.data.as_bytes()[package.chunk.start..package.chunk.start + package.header_size]
                .to_vec()
        };
        head[4..8].copy_from_slice(&(self.size as u32).to_le_bytes());
        for (slot, &unit) in head[12..268]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(&package.name)
        {
            slot.copy_from_slice(&unit.to_le_bytes());
        }
        let mut at = head.len();
        for chunk in &self.chunks {
            at += match chunk {
                PackageChunk::TypeStrings => {
                    head[268..272].copy_from_slice(&(at as u32).to_le_bytes());
                    self.type_strings.size
                }
                PackageChunk::KeyStrings => {
                    head[276..280].copy_from_slice(&(at as u32).to_le_bytes());
                    self.key_strings.size
                }
                PackageChunk::Spec(i) => package.type_specs[*i].size(),
                PackageChunk::Type(i) => self.types[*i].size,
                PackageChunk::Raw(range) => range.len(),
            };
        }
        out.write_all(&head)?;
        for chunk in &self.chunks {
            match chunk {
                PackageChunk::TypeStrings => self.type_strings.write(out)?,
                PackageChunk::KeyStrings => self.key_strings.write(out)?,
                PackageChunk::Spec(i) => package.type_specs[*i].write(out)?,
                PackageChunk::Type(i) => self.types[*i].write(out)?,
                PackageChunk::Raw(range) => {
                    out.write_all(&package.data.as_bytes()[range.clone()])?;
                }
            }
        }
        Ok(())
    }
}
