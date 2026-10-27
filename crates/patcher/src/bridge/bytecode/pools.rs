// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::phase::with_ctx;
use boltffi::export;

#[export]
pub fn is_added_dex(d: u32) -> bool {
    with_ctx(|ctx| ctx.apk().is_added_dex(d as usize))
}
