// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{HEADER_LEN, TypeSpec};
use crate::buf::{write_u16, write_u32};
use crate::chunk::write_header;
use crate::error::Result;
use crate::resources::RES_TABLE_TYPE_SPEC;
use std::io::Write;

impl TypeSpec {
    pub(in crate::resources) fn size(&self) -> usize {
        if self.chunk.is_empty() {
            HEADER_LEN + self.len() * 4
        } else {
            self.chunk.len() + self.extra.len() * 4
        }
    }

    pub(in crate::resources) fn write(&self, out: &mut dyn Write) -> Result<()> {
        if !self.chunk.is_empty() && self.extra.is_empty() {
            out.write_all(&self.data.as_bytes()[self.chunk.clone()])?;
            return Ok(());
        }
        let mut head = if self.chunk.is_empty() {
            let mut head = Vec::with_capacity(HEADER_LEN);
            write_header(
                &mut head,
                RES_TABLE_TYPE_SPEC,
                HEADER_LEN as u16,
                self.size(),
            );
            head.push(self.id);
            head.push(0);
            write_u16(&mut head, 0);
            write_u32(&mut head, self.len() as u32);
            head
        } else {
            self.data.as_bytes()[self.chunk.start..self.flags_start].to_vec()
        };
        head[4..8].copy_from_slice(&(self.size() as u32).to_le_bytes());
        head[12..16].copy_from_slice(&(self.len() as u32).to_le_bytes());
        out.write_all(&head)?;
        if self.raw_len > 0 {
            out.write_all(
                &self.data.as_bytes()[self.flags_start..self.flags_start + self.raw_len * 4],
            )?;
        }
        let mut extra = Vec::with_capacity(self.extra.len() * 4);
        for flag in &self.extra {
            write_u32(&mut extra, *flag);
        }
        out.write_all(&extra)?;
        if !self.chunk.is_empty() {
            out.write_all(
                &self.data.as_bytes()[self.flags_start + self.raw_len * 4..self.chunk.end],
            )?;
        }
        Ok(())
    }
}
