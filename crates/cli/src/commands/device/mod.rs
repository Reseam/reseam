// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands that put a patched app on a device over adb.

mod adb;
mod install;
mod mount;

pub use install::run_install;
pub use mount::{run_mount, run_unmount};
