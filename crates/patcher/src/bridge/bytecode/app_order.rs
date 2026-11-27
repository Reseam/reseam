// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_dex::MethodKind;

use crate::bridge::dex::types::AppPosition;
use crate::bridge::handles::{class_location, method_location};
use crate::bridge::phase::with_ctx;
use crate::bridge::run_state::checked;
use crate::context::ClassLocation;

#[export]
pub fn method_app_position(m: u32) -> Option<AppPosition> {
    let location = method_location(m)?;
    with_ctx(|ctx| {
        let member_index = match location.kind {
            MethodKind::Direct => location.method_idx,
            MethodKind::Virtual => {
                let counts = checked(ctx.read_class_counts(ClassLocation {
                    dex_idx: location.dex_idx,
                    class_idx: location.class_idx,
                }))?;
                counts.direct_methods as usize + location.method_idx
            }
        };
        Some(AppPosition {
            dex: location.dex_idx as u32,
            class_def: location.class_idx as u32,
            member: member_index as u32,
        })
    })
}

#[export]
pub fn class_app_position(c: u32) -> Option<AppPosition> {
    let location = class_location(c)?;
    Some(AppPosition {
        dex: location.dex_idx as u32,
        class_def: location.class_idx as u32,
        member: 0,
    })
}
