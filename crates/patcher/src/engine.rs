// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod index;
mod plan;
mod run;

pub use index::PatchIndex;
pub use plan::PatchSelection;
pub(crate) use plan::ResolvedPlan;
pub use run::{apply_patches, validate_patches};

pub use reseam_model::{PatchResult, PatchStatus, ProgressEvent};
