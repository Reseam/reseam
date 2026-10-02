// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{RES_TABLE_TYPE, ResPackage, ResourceTable, TABLE_HEADER_LEN, TableChunk};
use crate::buf::write_u32;
use crate::chunk::write_header;
use crate::error::{Result, invalid};
use std::fs::File;
use std::io::{BufWriter, Write};

impl ResourceTable {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_to(&mut out)?;
        Ok(out)
    }

    pub(crate) fn serialize_spooled(&self) -> Result<File> {
        let mut file = reseam_storage::temporary_file()?;
        let mut out = BufWriter::with_capacity(1 << 20, &mut file);
        self.write_to(&mut out)?;
        out.flush()?;
        drop(out);
        Ok(file)
    }

    /// Streams the table, returning I/O errors or errors for unrepresentable sizes.
    /// Parsed input pages are released after writing and remain readable from
    /// their backing file for later edits or writes.
    pub fn write_to(&self, out: &mut dyn Write) -> Result<()> {
        let global = self.global_strings.plan()?;
        let packages = self
            .packages
            .iter()
            .map(ResPackage::plan)
            .collect::<Result<Vec<_>>>()?;
        let add_strings = !self
            .chunks
            .iter()
            .any(|chunk| matches!(chunk, TableChunk::Strings))
            && !self.global_strings.is_empty();
        let header_size = self.header.len().max(TABLE_HEADER_LEN);
        let size = header_size
            + self
                .chunks
                .iter()
                .map(|chunk| match chunk {
                    TableChunk::Strings => global.size,
                    TableChunk::Package(i) => packages[*i].size,
                    TableChunk::Raw(range) => range.len(),
                })
                .sum::<usize>()
            + if add_strings { global.size } else { 0 };
        let size =
            u32::try_from(size).map_err(|_| invalid("resource table", "table exceeds 4 GiB"))?;
        let mut head = if self.header.is_empty() {
            let mut head = Vec::new();
            write_header(
                &mut head,
                RES_TABLE_TYPE,
                TABLE_HEADER_LEN as u16,
                size as usize,
            );
            write_u32(&mut head, self.packages.len() as u32);
            head
        } else {
            self.data.as_bytes()[self.header.clone()].to_vec()
        };
        head[4..8].copy_from_slice(&size.to_le_bytes());
        head[8..12].copy_from_slice(&(self.packages.len() as u32).to_le_bytes());
        out.write_all(&head)?;
        if add_strings {
            global.write(out)?;
        }
        for chunk in &self.chunks {
            match chunk {
                TableChunk::Strings => global.write(out)?,
                TableChunk::Package(i) => packages[*i].write(out)?,
                TableChunk::Raw(range) => out.write_all(&self.data.as_bytes()[range.clone()])?,
            }
        }
        out.write_all(&self.data.as_bytes()[self.suffix.clone()])?;
        self.data.release_pages();
        Ok(())
    }
}
