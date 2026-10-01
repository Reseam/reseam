// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    CHUNK_STRING_POOL, FLAG_UTF8, HEADER_LEN, StringEncoding, StringPool, UTF8_LENGTH_MASK,
};
use crate::buf::{read_u32_le, write_u16, write_u32};
use crate::error::{Result, invalid};
use std::io::Write;
use std::ops::Range;

impl StringPool {
    pub(crate) fn plan(&self) -> Result<PoolPlan<'_>> {
        let data = self.data.as_bytes();
        if self.overrides.is_empty() && self.owned.is_empty() && !self.chunk.is_empty() {
            return Ok(PoolPlan {
                pool: self,
                size: self.chunk.len(),
                rebuilt: None,
            });
        }
        let count = self.len();
        let raw_start = if self.raw_len == 0 {
            self.styles_start
        } else {
            self.strings_start
        };
        let raw_region = raw_start..self.styles_start;
        let mut extra = Vec::new();
        let mut offsets = Vec::with_capacity(count);
        for i in 0..count as u32 {
            if (i as usize) < self.raw_len && !self.overrides.contains_key(&i) {
                offsets.push(read_u32_le(
                    data,
                    self.offsets_start + i as usize * 4,
                    "string offset",
                )?);
                continue;
            }
            offsets.push(
                u32::try_from(raw_region.len() + extra.len())
                    .map_err(|_| invalid("string pool", "string data exceeds 4 GiB"))?,
            );
            let s = self.get(i)?.expect("planned indices are within the pool");
            if self.encoding == StringEncoding::Utf8 {
                encode_utf8(&mut extra, &s);
            } else {
                encode_utf16(&mut extra, &s);
            }
        }
        let has_styles = self.style_count > 0;
        if has_styles {
            let padded = (raw_region.len() + extra.len()).div_ceil(4) * 4;
            extra.resize(padded - raw_region.len(), 0);
        }
        let header_size = if self.chunk.is_empty() {
            HEADER_LEN
        } else {
            self.offsets_start - self.chunk.start
        };
        let index_end = self.offsets_start + (self.raw_len + self.style_count) * 4;
        let gap_end = if self.raw_len == 0 {
            self.styles_start
        } else {
            self.strings_start
        };
        let gap = index_end..gap_end.max(index_end);
        let strings_start = header_size + (count + self.style_count) * 4 + gap.len();
        let styles_start = strings_start + raw_region.len() + extra.len();
        let style_data = self.styles_start..self.chunk.end;
        Ok(PoolPlan {
            pool: self,
            size: (styles_start + style_data.len() + 3) & !3,
            rebuilt: Some(RebuiltPool {
                offsets,
                extra,
                gap,
                raw_region,
                style_data,
                strings_start,
                styles_start: if has_styles { styles_start } else { 0 },
            }),
        })
    }
}

pub(crate) struct PoolPlan<'a> {
    pool: &'a StringPool,
    pub size: usize,
    rebuilt: Option<RebuiltPool>,
}

struct RebuiltPool {
    offsets: Vec<u32>,
    extra: Vec<u8>,
    gap: Range<usize>,
    raw_region: Range<usize>,
    style_data: Range<usize>,
    strings_start: usize,
    styles_start: usize,
}

impl PoolPlan<'_> {
    pub(crate) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let pool = self.pool;
        let data = pool.data.as_bytes();
        let Some(plan) = &self.rebuilt else {
            out.write_all(&data[pool.chunk.clone()])?;
            return Ok(());
        };
        let mut header = if pool.chunk.is_empty() {
            let mut head = Vec::with_capacity(HEADER_LEN);
            write_u16(&mut head, CHUNK_STRING_POOL);
            write_u16(&mut head, HEADER_LEN as u16);
            head.resize(HEADER_LEN, 0);
            head[16..20].copy_from_slice(
                &(if pool.encoding == StringEncoding::Utf8 {
                    FLAG_UTF8
                } else {
                    0
                })
                .to_le_bytes(),
            );
            head
        } else {
            data[pool.chunk.start..pool.offsets_start].to_vec()
        };
        let flags = read_u32_le(&header, 16, "string pool flags")? & !1;
        header[4..8].copy_from_slice(
            &u32::try_from(self.size)
                .map_err(|_| invalid("string pool", "pool exceeds 4 GiB"))?
                .to_le_bytes(),
        );
        header[8..12].copy_from_slice(&(plan.offsets.len() as u32).to_le_bytes());
        header[12..16].copy_from_slice(&(pool.style_count as u32).to_le_bytes());
        header[16..20].copy_from_slice(&flags.to_le_bytes());
        header[20..24].copy_from_slice(&(plan.strings_start as u32).to_le_bytes());
        header[24..28].copy_from_slice(&(plan.styles_start as u32).to_le_bytes());
        for offset in &plan.offsets {
            write_u32(&mut header, *offset);
        }
        out.write_all(&header)?;
        let style_offsets = pool.offsets_start + pool.raw_len * 4;
        out.write_all(&data[style_offsets..style_offsets + pool.style_count * 4])?;
        out.write_all(&data[plan.gap.clone()])?;
        out.write_all(&data[plan.raw_region.clone()])?;
        out.write_all(&plan.extra)?;
        out.write_all(&data[plan.style_data.clone()])?;
        let written = header.len()
            + pool.style_count * 4
            + plan.gap.len()
            + plan.raw_region.len()
            + plan.extra.len()
            + plan.style_data.len();
        out.write_all(&[0; 3][..self.size - written])?;
        Ok(())
    }
}

fn encode_utf8(out: &mut Vec<u8>, s: &str) {
    encode_len_u8(out, s.encode_utf16().count());
    encode_len_u8(out, s.len());
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn encode_len_u8(out: &mut Vec<u8>, len: usize) {
    if len > 0x7F {
        out.push(((len & UTF8_LENGTH_MASK) >> 8) as u8 | 0x80);
    }
    out.push((len & 0xFF) as u8);
}

fn encode_utf16(out: &mut Vec<u8>, s: &str) {
    let len = s.encode_utf16().count();
    if len > 0x7FFF {
        write_u16(out, ((len >> 16) as u16) | 0x8000);
    }
    write_u16(out, (len & 0xFFFF) as u16);
    for unit in s.encode_utf16() {
        write_u16(out, unit);
    }
    write_u16(out, 0);
}
