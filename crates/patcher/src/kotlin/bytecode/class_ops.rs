// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::kotlin::handles::checked;
use boltffi::export;
use reseam_apk::reseam_dex::{AccessFlags, DexFile};

use crate::context::ClassLocation;
use crate::kotlin::handles::{
    alloc_class, changed, class_location, forget_class, record_failure, with_class_mut, with_ctx,
};
use crate::kotlin::link::link_descriptors;

use super::logged;

fn with_class_header<R>(
    c: u32,
    f: impl FnOnce(&mut DexFile, ClassLocation) -> Option<R>,
) -> Option<R> {
    let location = class_location(c)?;
    changed();
    with_ctx(|ctx| f(ctx.dex_file_mut(location.dex_idx)?, location))
}

#[export]
pub fn set_class_access_flags(c: u32, flags: u32) {
    with_class_header(c, |dex, loc| {
        checked(dex.class_mut(loc.class_idx).map(Some))?.access_flags =
            AccessFlags::from_bits_retain(flags);
        Some(())
    });
}

#[export]
pub fn set_superclass(c: u32, superclass: String) {
    link_descriptors([superclass.as_str()]);
    with_class_header(c, |dex, loc| {
        checked(dex.set_superclass(loc.class_idx, &superclass).map(Some))
    });
}

#[export]
pub fn add_interface(c: u32, interface_descriptor: String) {
    link_descriptors([interface_descriptor.as_str()]);
    with_class_mut(c, |dex, loc| {
        let interface = logged("intern interface", dex.intern_type(&interface_descriptor))?;
        logged("add interface", dex.class_mut(loc.class_idx))?
            .interfaces
            .push(interface);
        Ok(Some(()))
    });
}

#[export]
pub fn remove_class(c: u32) {
    with_class_header(c, |dex, loc| {
        let class_type = dex.class_header(loc.class_idx).class_type;
        let removed = checked(dex.remove_class(class_type).map(Some))?;
        forget_class(loc);
        Some(removed)
    });
}

#[export]
pub fn create_class(dex_index: u32, descriptor: String, flags: u32, superclass: String) -> u32 {
    link_descriptors([superclass.as_str()]);
    changed();
    with_ctx(|ctx| {
        let Some(dex) = ctx.dex_file_mut(dex_index as usize) else {
            record_failure(format!("invalid DEX index {dex_index}"));
            return None;
        };
        let class_idx = checked(
            dex.create_class(
                &descriptor,
                AccessFlags::from_bits_retain(flags),
                Some(&superclass),
            )
            .map(Some),
        )?;
        Some(alloc_class(ClassLocation {
            dex_idx: dex_index as usize,
            class_idx,
        }))
    })
    .unwrap_or(0)
}

#[export]
pub fn definal_class(c: u32) {
    with_class_mut(c, |dex, loc| {
        logged("definal class", dex.class_mut(loc.class_idx))?.definal();
        Ok(Some(()))
    });
}

#[export]
pub fn superclass_chain(c: u32) -> Vec<u32> {
    let Some(location) = class_location(c) else {
        return Vec::new();
    };
    with_ctx(|ctx| ctx.superclass_chain(location))
        .into_iter()
        .map(alloc_class)
        .collect()
}
