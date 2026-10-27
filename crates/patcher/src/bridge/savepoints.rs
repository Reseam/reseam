// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_journal::Savepoint;

use super::dex::pool_copies;
use super::{changes, handles};
use crate::error::Result as PatcherResult;

/// What the engine asks of the run state at a savepoint.
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SavepointAction {
    Open,
    Release,
    Rollback,
}

/// Opens `savepoint` over the bridge's run state and the patch runtime's.
pub(crate) fn open_savepoint(savepoint: Savepoint) -> PatcherResult<()> {
    handles::with_handles(|handles| handles.open_savepoint(savepoint));
    pool_copies::open_savepoint(savepoint);
    runtime_savepoint(SavepointAction::Open, savepoint)
}

pub(crate) fn release_savepoint(savepoint: Savepoint) -> PatcherResult<()> {
    handles::with_handles(|handles| handles.release_savepoint(savepoint));
    pool_copies::release_savepoint(savepoint);
    runtime_savepoint(SavepointAction::Release, savepoint)
}

/// Undoes the bridge's and the patch runtime's changes since `savepoint`.
/// Everything a patch runtime cached about the app may be stale after it.
pub(crate) fn rollback_to(savepoint: Savepoint) -> PatcherResult<()> {
    handles::with_handles(|handles| handles.rollback_to(savepoint));
    pool_copies::rollback_to(savepoint);
    changes::changed();
    runtime_savepoint(SavepointAction::Rollback, savepoint)
}

/// The savepoints open now, which a patch runtime created later must open too.
#[cfg(feature = "kotlin")]
pub(super) fn open_savepoints() -> Vec<Savepoint> {
    handles::with_handles(|handles| handles.savepoints())
}

#[cfg_attr(
    not(any(feature = "kotlin", feature = "browser")),
    expect(
        clippy::unnecessary_wraps,
        reason = "a host without a patch runtime has none to tell"
    )
)]
fn runtime_savepoint(action: SavepointAction, savepoint: Savepoint) -> PatcherResult<()> {
    #[cfg(feature = "kotlin")]
    crate::bridge::runtime::callback::savepoint(action, savepoint)?;
    #[cfg(all(feature = "browser", not(feature = "kotlin")))]
    crate::bridge::runtime::browser::savepoint(action, savepoint)?;
    Ok(())
}
