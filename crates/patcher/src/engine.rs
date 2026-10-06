// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod index;
mod plan;
mod run;

pub use index::PatchIndex;
pub use plan::PatchSelection;
pub(crate) use plan::ResolvedPlan;
pub use run::{Delivery, apply_patches, validate_patches};

pub use reseam_model::{PatchResult, PatchStatus, ProgressEvent};
