// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod bundle;
pub mod context;
pub mod dependency;
pub mod engine;
pub mod error;
#[cfg(feature = "kotlin")]
pub mod kotlin;
pub mod log;
pub mod options;
pub mod patch;

pub use reseam_apk;
pub use reseam_apk::reseam_dex;

pub mod prelude {
    pub use crate::context::PatchContext;
    pub use crate::engine::ExecutionPlan;
    pub use crate::error::{PatcherError, Result};
    pub use crate::log::{LogEntry, LogLevel, PatchLog};
    pub use crate::options::{OptionDeclaration, OptionType, OptionValue, PatchOptions};
    pub use crate::patch::{Compatibility, Patch};
    pub use reseam_patcher_macros::reseam_patch;
}

pub use reseam_patcher_macros::reseam_patch;
