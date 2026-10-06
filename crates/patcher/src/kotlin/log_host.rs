// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;

use super::handles::with_ctx;

#[export]
pub fn log_info(msg: String) {
    with_ctx(|ctx| ctx.log().info(msg));
}

#[export]
pub fn log_warn(msg: String) {
    with_ctx(|ctx| ctx.log().warn(msg));
}

#[export]
pub fn log_debug(msg: String) {
    with_ctx(|ctx| ctx.log().debug(msg));
}
