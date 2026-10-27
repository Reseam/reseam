// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use super::SECTION;
use super::cursor::Cursor;
use super::{
    BaselineProfile, DexMetadata, DexProfile, METADATA_MAGIC, METADATA_V002, MethodFlags,
    PROFILE_MAGIC, PROFILE_V010,
};
use crate::error::{Result, invalid, malformed};

const INLINE_CACHE_MISSING_TYPES: u8 = 6;
const INLINE_CACHE_MEGAMORPHIC: u8 = 7;

impl BaselineProfile {
    /// Parses a version 010 `baseline.prof` and, when given, the version 002
    /// `baseline.profm` beside it.
    pub(crate) fn parse(profile: &[u8], metadata: Option<&[u8]>) -> Result<Self> {
        let mut file = Cursor::new(profile);
        file.header(PROFILE_MAGIC, PROFILE_V010)?;
        let count = file.u8()?;
        let (inflated, deflated) = (file.u32()?, file.u32()?);
        let body = file.inflated(inflated, deflated)?;
        let mut body = Cursor::new(&body);
        let headers = (0..count)
            .map(|_| {
                let key_len = body.u16()?;
                let class_count = body.u16()?;
                let hot_size = body.u32()?;
                let checksum = body.u32()?;
                let method_count = body.u32()?;
                Ok((
                    body.string(key_len.into())?,
                    class_count,
                    hot_size,
                    checksum,
                    method_count,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut dex = headers
            .into_iter()
            .map(|(name, class_count, hot_size, checksum, method_count)| {
                let mut methods = read_hot_methods(&mut body, hot_size as usize)?;
                let classes = body.ascending(class_count.into())?;
                let bitmap = body.bytes((method_count as usize * 2).div_ceil(8))?;
                let bit = |index: usize| bitmap[index / 8] & (1 << (index % 8)) != 0;
                for method in 0..method_count {
                    let mut flags = MethodFlags::empty();
                    flags.set(MethodFlags::STARTUP, bit(method as usize));
                    flags.set(
                        MethodFlags::POST_STARTUP,
                        bit((method + method_count) as usize),
                    );
                    if !flags.is_empty() {
                        *methods.entry(method).or_insert(MethodFlags::empty()) |= flags;
                    }
                }
                Ok(DexProfile {
                    name,
                    checksum,
                    method_count,
                    classes,
                    methods,
                    metadata: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if !body.is_at_end() {
            return Err(malformed(SECTION, body.pos, "trailing profile data"));
        }
        if let Some(metadata) = metadata {
            read_metadata(metadata, &mut dex)?;
        }
        Ok(Self { dex })
    }
}

fn read_hot_methods(body: &mut Cursor<'_>, size: usize) -> Result<BTreeMap<u32, MethodFlags>> {
    let end = body.pos + size;
    let mut methods = BTreeMap::new();
    let mut last = 0u32;
    while body.pos < end {
        last += u32::from(body.u16()?);
        methods.insert(last, MethodFlags::HOT);
        for _ in 0..body.u16()? {
            skip_inline_cache(body)?;
        }
    }
    if body.pos != end {
        return Err(malformed(
            SECTION,
            body.pos,
            "hot method region overruns its size",
        ));
    }
    Ok(methods)
}

fn skip_inline_cache(body: &mut Cursor<'_>) -> Result<()> {
    body.u16()?;
    let dex_pcs = body.u8()?;
    if matches!(
        dex_pcs,
        INLINE_CACHE_MISSING_TYPES | INLINE_CACHE_MEGAMORPHIC
    ) {
        return Ok(());
    }
    for _ in 0..dex_pcs {
        body.u8()?;
        let classes = body.u8()?;
        body.bytes(usize::from(classes) * 2)?;
    }
    Ok(())
}

fn read_metadata(metadata: &[u8], dex: &mut [DexProfile]) -> Result<()> {
    let mut file = Cursor::new(metadata);
    file.header(METADATA_MAGIC, METADATA_V002)?;
    let count = file.u16()?;
    let (inflated, deflated) = (file.u32()?, file.u32()?);
    let body = file.inflated(inflated, deflated)?;
    let mut body = Cursor::new(&body);
    for _ in 0..count {
        body.u16()?;
        let key_len = body.u16()?;
        let key = body.string(key_len.into())?;
        let type_count = body.u32()?;
        let class_count = body.u16()?;
        let class_defs = body.ascending(class_count.into())?;
        let name = key.rsplit(['!', ':']).next().unwrap_or(&key);
        let profile = dex
            .iter_mut()
            .find(|profile| profile.name == name)
            .ok_or_else(|| invalid(SECTION, format!("metadata names unknown DEX {key}")))?;
        profile.metadata = Some(DexMetadata {
            type_count,
            class_defs,
        });
    }
    Ok(())
}
