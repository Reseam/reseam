// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::IdRecord;
use crate::error::Result;
use crate::model::{FieldId, MethodId, ProtoIdx, Prototype, StringIdx, TypeIdx, TypeList};
use crate::read::type_list::{read_type_list, validate_type_list};
use reseam_bytes::{u16_at, u32_at};

impl IdRecord for StringIdx {
    const SIZE: usize = 4;
    type Key<'a> = u32;

    fn read(buf: &[u8], off: usize) -> Self {
        StringIdx(u32_at(buf, off))
    }

    fn key(&self) -> u32 {
        self.0
    }
}

impl IdRecord for Prototype {
    const SIZE: usize = 12;
    type Key<'a> = (TypeIdx, &'a [TypeIdx]);

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
        let params_off = u32_at(buf, off + 8);
        if params_off != 0 {
            validate_type_list(buf, params_off as usize)?;
        }
        Ok(())
    }

    fn key(&self) -> (TypeIdx, &[TypeIdx]) {
        (self.return_type, &self.parameters)
    }
}

impl IdRecord for FieldId {
    const SIZE: usize = 8;
    type Key<'a> = &'a Self;

    fn read(buf: &[u8], off: usize) -> Self {
        FieldId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            type_: TypeIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn key(&self) -> &Self {
        self
    }
}

impl IdRecord for MethodId {
    const SIZE: usize = 8;
    type Key<'a> = &'a Self;

    fn read(buf: &[u8], off: usize) -> Self {
        MethodId {
            class: TypeIdx(u32::from(u16_at(buf, off))),
            proto: ProtoIdx(u32::from(u16_at(buf, off + 2))),
            name: StringIdx(u32_at(buf, off + 4)),
        }
    }

    fn key(&self) -> &Self {
        self
    }
}
