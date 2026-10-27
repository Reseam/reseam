// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;

use crate::bridge::phase::with_patch;

#[export]
pub fn log_info(msg: String) {
    with_patch(|scope| scope.log.info(msg));
}

#[export]
pub fn log_warn(msg: String) {
    with_patch(|scope| scope.log.warn(msg));
}

#[export]
pub fn log_debug(msg: String) {
    with_patch(|scope| scope.log.debug(msg));
}
