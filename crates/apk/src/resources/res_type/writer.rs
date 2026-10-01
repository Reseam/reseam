// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{FLAG_OFFSET16, FLAG_SPARSE, HEADER_LEN, NO_ENTRY, ResType};
use crate::buf::write_u32;
use crate::chunk::write_header;
use crate::error::{Result, invalid};
use crate::resources::{RES_TABLE_TYPE_TYPE, entry};
use std::io::Write;
impl ResType {
    pub(in crate::resources) fn plan(&self) -> Result<TypePlan<'_>> {
        if self.overlay.is_empty() && self.len == self.raw_len && !self.chunk.is_empty() {
            return Ok(TypePlan {
                res_type: self,
                size: self.chunk.len(),
                rebuilt: None,
            });
        }
        let config = match self.config() {
            [] => &[4, 0, 0, 0],
            config => config,
        };
        let raw_data = &self.data.as_bytes()[self.entries_start..self.chunk.end];
        let mut overlay_bytes = Vec::new();
        let mut offsets = Vec::with_capacity(self.len);
        for i in 0..self.len {
            let offset = match self.overlay.get(&(i as u32)) {
                Some(Some(entry)) => {
                    let padding = (4 - (raw_data.len() + overlay_bytes.len()) % 4) % 4;
                    overlay_bytes.resize(overlay_bytes.len() + padding, 0);
                    let offset = u32::try_from(raw_data.len() + overlay_bytes.len())
                        .map_err(|_| invalid("res type", "entry data exceeds 4 GiB"))?;
                    entry::serialize_entry(&mut overlay_bytes, entry);
                    Some(offset)
                }
                Some(None) => None,
                None => self.raw_offset(i)?,
            };
            offsets.push(offset);
        }
        let entries_start = HEADER_LEN + config.len() + self.len * 4;
        let size = entries_start + raw_data.len() + overlay_bytes.len();
        u32::try_from(size).map_err(|_| invalid("res type", "chunk exceeds 4 GiB"))?;
        Ok(TypePlan {
            res_type: self,
            size,
            rebuilt: Some(RebuiltType {
                config,
                offsets,
                raw_data,
                overlay_bytes,
                entries_start,
            }),
        })
    }
}

pub(in crate::resources) struct TypePlan<'a> {
    res_type: &'a ResType,
    pub size: usize,
    rebuilt: Option<RebuiltType<'a>>,
}

struct RebuiltType<'a> {
    config: &'a [u8],
    offsets: Vec<Option<u32>>,
    raw_data: &'a [u8],
    overlay_bytes: Vec<u8>,
    entries_start: usize,
}

impl TypePlan<'_> {
    pub(in crate::resources) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let res_type = self.res_type;
        let Some(plan) = &self.rebuilt else {
            out.write_all(&res_type.data.as_bytes()[res_type.chunk.clone()])?;
            return Ok(());
        };
        let mut head = Vec::with_capacity(plan.entries_start);
        write_header(
            &mut head,
            RES_TABLE_TYPE_TYPE,
            (HEADER_LEN + plan.config.len()) as u16,
            self.size,
        );
        head.push(res_type.id);
        let metadata = if res_type.chunk.is_empty() {
            [0; 3]
        } else {
            let bytes =
                &res_type.data.as_bytes()[res_type.chunk.start + 9..res_type.chunk.start + 12];
            [
                bytes[0] & !(FLAG_SPARSE | FLAG_OFFSET16),
                bytes[1],
                bytes[2],
            ]
        };
        head.extend_from_slice(&metadata);
        write_u32(&mut head, res_type.len as u32);
        write_u32(&mut head, plan.entries_start as u32);
        head.extend_from_slice(plan.config);
        for offset in &plan.offsets {
            write_u32(&mut head, offset.unwrap_or(NO_ENTRY));
        }
        out.write_all(&head)?;
        out.write_all(plan.raw_data)?;
        out.write_all(&plan.overlay_bytes)?;
        Ok(())
    }
}
