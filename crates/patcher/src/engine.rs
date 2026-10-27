// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod plan;
mod run;

pub use plan::{Delivery, PatchSelection, ResolvedPlan};
pub use run::{AppliedRun, apply_patches, validate_patches};

pub use reseam_model::{PatchResult, PatchStatus, RunEvent};
