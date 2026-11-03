// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::estimated_ir_bytes;
use reseam_model::{ApplyDiagnostics, MaterializationStats, MemoryBreakdown, PatchPhase};
use reseam_patcher::bundle::PatchBundle;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{self, AppliedRun, PlanOptions, ResolvedPlan, Trace};
use reseam_patcher::{Patch, PatchSpec};

use crate::apk::OpenedApk;
use crate::bundles::TrustedArchive;
use crate::error::Result;
use crate::metrics::current_rss_bytes;
use crate::progress::RunProgress;

/// Loads the code of the patches `plan` executes from each archive.
pub(crate) fn load_executed(
    archives: Vec<TrustedArchive>,
    plan: &ResolvedPlan,
    progress: &mut RunProgress<'_>,
) -> Result<Vec<PatchBundle>> {
    let executed = executed_patches(&archives, plan);
    progress.measure(PatchPhase::LoadBundles, |_| {
        archives
            .into_iter()
            .zip(&executed)
            .map(|(archive, patches)| archive.load(patches))
            .collect()
    })
}

/// The ids of the patches `plan` executes, per archive. Plan positions follow
/// the archives' catalogs concatenated in order.
fn executed_patches(archives: &[TrustedArchive], plan: &ResolvedPlan) -> Vec<HashSet<String>> {
    let mut positions = 0..;
    archives
        .iter()
        .map(|archive| {
            archive
                .catalog()
                .iter()
                .zip(positions.by_ref())
                .filter(|&(_, idx)| plan.executes(idx))
                .map(|(spec, _)| spec.reference().id.clone())
                .collect()
        })
        .collect()
}

/// Applies `plan` to `opened`.
pub(crate) fn apply<'b>(
    opened: &mut OpenedApk,
    bundles: &'b [PatchBundle],
    plan: ResolvedPlan,
    options: &PlanOptions,
    trace: Trace,
    progress: &mut RunProgress<'_>,
) -> Result<AppliedRun<'b>> {
    let specs: Vec<&PatchSpec> = bundles.iter().flat_map(PatchBundle::catalog).collect();
    let patches: Vec<&Patch> = bundles.iter().flat_map(PatchBundle::patches).collect();
    let extension_paths: Vec<_> = bundles
        .iter()
        .flat_map(|bundle| bundle.extension_dex().iter().cloned())
        .collect();
    let mut ctx = PatchContext::new(&mut opened.apk, ExtensionSet::load(&extension_paths)?);
    let applied = progress.measure(PatchPhase::ApplyPatches, |progress| {
        engine::apply_patches(&mut ctx, &specs, plan, options, &patches, trace, |event| {
            progress.emit(event);
        })
    })?;
    progress.set_apply_diagnostics(apply_diagnostics(&ctx));
    Ok(applied)
}

fn apply_diagnostics(ctx: &PatchContext<'_>) -> ApplyDiagnostics {
    let dex = ctx.apk().dex().memory_breakdown();
    ApplyDiagnostics {
        rss_bytes: current_rss_bytes(),
        dex: MemoryBreakdown {
            raw_buffer_bytes: dex.raw_buffer_bytes,
            string_pool_bytes: dex.string_pool_bytes,
            string_count: dex.string_count,
            id_table_bytes: dex.id_table_bytes,
            class_def_bytes: dex.class_def_bytes,
            materialized_ir_bytes: estimated_ir_bytes(&dex.materialized),
            materialized: MaterializationStats {
                total_classes: dex.materialized.total_classes,
                resolved_classes: dex.materialized.resolved_classes,
                methods: dex.materialized.methods,
                instructions: dex.materialized.instructions,
            },
        },
        jvm: reseam_patcher::jvm_heap_stats(),
    }
}
