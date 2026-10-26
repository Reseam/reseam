// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use super::IdRecord;
use crate::error::Result;
use crate::read::type_list::{read_type_list, validate_type_list};
use crate::read::{u16_at, u32_at};
use crate::types::{FieldId, MethodId, ProtoIdx, Prototype, StringIdx, TypeIdx, TypeList};

fn check(buf: &[u8], off: usize, size: usize) -> Result<()> {
    crate::error::require_len(buf, off, size, "id table entry")
}

impl IdRecord for StringIdx {
    const SIZE: usize = 4;

    fn read(buf: &[u8], off: usize) -> Self {
        StringIdx(u32_at(buf, off))
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl IdRecord for Prototype {
    const SIZE: usize = 12;

    fn read(buf: &[u8], off: usize) -> Self {
        let params_off = u32_at(buf, off + 8) as usize;
        let parameters = if params_off == 0 {
            TypeList::new()
        } else {
            read_type_list(buf, params_off)
        };
        Prototype {
            shorty: StringIdx(u32_at(buf, off)),
            return_type: TypeIdx(u32_at(buf, off + 4)),
            parameters,
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)?;
        let params_off = u32_at(buf, off + 8);
        if params_off != 0 {
            validate_type_list(buf, params_off as usize)?;
        }
        Ok(())
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.return_type
            .cmp(&other.return_type)
            .then_with(|| self.parameters.cmp(&other.parameters))
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.return_type.hash(state);
        self.parameters.hash(state);
    }
}

impl IdRecord for FieldId {
    const SIZE: usize = 8;

    fn read(buf: &[u8], off: usize) -> Self {
        FieldId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            type_: TypeIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

impl IdRecord for MethodId {
    const SIZE: usize = 8;

    fn read(buf: &[u8], off: usize) -> Self {
        MethodId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            proto: ProtoIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn validate(buf: &[u8], off: usize) -> Result<()> {
        check(buf, off, Self::SIZE)
    }

    fn key_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    fn key_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}
