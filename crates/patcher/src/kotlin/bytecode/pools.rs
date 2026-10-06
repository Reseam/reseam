// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::kotlin::handles::checked;
use crate::kotlin::handles::{changed, method_location, record_failure, with_ctx};
use boltffi::export;
use reseam_apk::reseam_dex::{DexFile, StringIdx};

#[export]
pub fn dex_count() -> u32 {
    with_ctx(|ctx| ctx.dex().len() as u32)
}

#[export]
pub fn method_dex(m: u32) -> u32 {
    method_location(m).map_or(0, |loc| loc.dex_idx as u32)
}

#[export]
pub fn is_added_dex(d: u32) -> bool {
    with_ctx(|ctx| ctx.apk().is_added_dex(d as usize))
}

fn with_dex<R>(d: u32, f: impl FnOnce(&mut DexFile) -> Option<R>) -> Option<R> {
    changed();
    with_ctx(|ctx| {
        let Some(dex) = ctx.dex_file_mut(d as usize) else {
            record_failure(format!("invalid DEX index {d}"));
            return None;
        };
        f(dex)
    })
}

#[export]
pub fn intern_string(d: u32, s: String) -> u32 {
    with_dex(d, |dex| Some(dex.intern_string(&s).0)).unwrap_or(0)
}

#[export]
pub fn intern_type(d: u32, descriptor: String) -> u32 {
    with_dex(d, |dex| {
        checked(dex.intern_type(&descriptor).map(|idx| Some(idx.0)))
    })
    .unwrap_or(0)
}

#[export]
pub fn intern_proto(d: u32, proto: String) -> u32 {
    with_dex(d, |dex| {
        checked(dex.intern_proto(&proto).map(Some)).map(|idx| idx.0)
    })
    .unwrap_or(0)
}

#[export]
pub fn intern_method(d: u32, descriptor: String, name: String, proto: String) -> u32 {
    with_dex(d, |dex| {
        checked(dex.intern_method(&descriptor, &name, &proto).map(Some)).map(|idx| idx.0)
    })
    .unwrap_or(0)
}

#[export]
pub fn intern_field(d: u32, descriptor: String, name: String, field_type: String) -> u32 {
    with_dex(d, |dex| {
        checked(dex.intern_field(&descriptor, &name, &field_type).map(Some)).map(|idx| idx.0)
    })
    .unwrap_or(0)
}

fn with_dex_read<R: Default>(d: u32, f: impl FnOnce(&DexFile) -> R) -> R {
    with_ctx(|ctx| {
        let Some(dex) = ctx.dex_file(d as usize) else {
            record_failure(format!("invalid DEX index {d}"));
            return R::default();
        };
        f(dex)
    })
}

#[export]
pub fn find_string_idx(d: u32, s: String) -> Option<u32> {
    with_dex_read(d, |dex| dex.find_string_idx(&s).map(|idx| idx.0))
}

#[export]
pub fn get_string(d: u32, idx: u32) -> String {
    with_dex_read(d, |dex| {
        if idx as usize >= dex.strings().len() {
            record_failure(format!("DEX {d} has no string {idx}"));
            return String::new();
        }
        dex.string(StringIdx(idx)).into_owned()
    })
}

#[export]
pub fn get_type_descriptor(d: u32, idx: u32) -> String {
    with_dex_read(d, |dex| {
        let Some(string) = dex.types().try_get(idx as usize) else {
            record_failure(format!("DEX {d} has no type {idx}"));
            return String::new();
        };
        if string.0 as usize >= dex.strings().len() {
            record_failure(format!(
                "DEX {d} type {idx} has invalid string {}",
                string.0
            ));
            return String::new();
        }
        dex.string(string).into_owned()
    })
}

#[export]
pub fn build_lookups(d: u32) {
    with_dex(d, |dex| {
        dex.build_lookups();
        Some(())
    });
}
