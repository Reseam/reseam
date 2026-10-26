// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::WriteLayout;
use tracing::warn;

use super::DexJob;
use crate::apk_file::dex_set::{DexIndex, DexSource};
use crate::apk_file::{ApkFile, ComponentIndex};
use crate::art_profile::{BaselineProfile, DexProfile, PROFILE_ENTRY};
use crate::error::Result;
use crate::zip::reader;

/// A component's baseline profile carried through a write. Rewritten DEX
/// files change their indices and checksum, so each keeps the profile of its
/// classes in the indices and checksum it is written with.
pub(super) struct ProfileRewrite {
    source: BaselineProfile,
    /// Profiles of the written DEX files, per source DEX.
    written: Vec<Vec<DexProfile>>,
}

impl ProfileRewrite {
    /// `None` when the component ships no baseline profile or one this engine
    /// cannot read, which is then written as it is.
    pub(super) fn new(
        apk: &ApkFile,
        component: ComponentIndex,
        jobs: &[DexJob],
    ) -> Result<Option<Self>> {
        let archive = apk.components[component.0].archive();
        if apk.components[component.0]
            .edits()
            .contains_key(PROFILE_ENTRY)
            || !reader::contains(archive, PROFILE_ENTRY)
        {
            return Ok(None);
        }
        let bytes = reader::read_entry(&mut archive.clone(), PROFILE_ENTRY)?;
        let source = match BaselineProfile::parse(&bytes, None) {
            Ok(source) => source,
            Err(error) => {
                warn!(%error, "baseline profile left as it is");
                return Ok(None);
            }
        };
        let written = source
            .dex
            .iter()
            .map(|profile| {
                let Some(index) = apk.profiled_dex(component, &profile.name) else {
                    return Vec::new();
                };
                let rewritten = jobs
                    .iter()
                    .any(|job| job.members.iter().any(|member| member.dex_index == index));
                if rewritten {
                    return Vec::new();
                }
                let dex = apk.dex.dex(index);
                vec![profile.rewrite(
                    dex,
                    &WriteLayout::unchanged(dex),
                    &profile.name,
                    profile.checksum,
                )]
            })
            .collect();
        Ok(Some(Self { source, written }))
    }

    /// Records a DEX entry as written. Members of a v041 container share one
    /// entry and checksum, which baseline profiles have no form for.
    pub(super) fn record(
        &mut self,
        apk: &ApkFile,
        component: ComponentIndex,
        job: &DexJob,
        layouts: &[WriteLayout],
        checksum: u32,
    ) {
        let ([member], [layout]) = (job.members.as_slice(), layouts) else {
            return;
        };
        let origin = apk.dex.origin(member.dex_index);
        let Some(position) = self
            .source
            .dex
            .iter()
            .position(|profile| profile.name == origin.name.as_str())
        else {
            return;
        };
        if apk.profiled_dex(component, origin.name.as_str()) != Some(member.dex_index) {
            return;
        }
        let dex = apk.dex.dex(member.dex_index);
        self.written[position].push(self.source.dex[position].rewrite(
            dex,
            layout,
            job.name.as_str(),
            checksum,
        ));
    }

    pub(super) fn finish(self) -> BaselineProfile {
        BaselineProfile {
            dex: self.written.into_iter().flatten().collect(),
        }
    }
}

impl ApkFile {
    /// The DEX parsed from entry `name` of `component`, which its baseline
    /// profile was recorded against.
    fn profiled_dex(&self, component: ComponentIndex, name: &str) -> Option<DexIndex> {
        self.dex.slots().find_map(|(index, _, origin)| {
            (origin.component == component
                && origin.kind == DexSource::Archive
                && origin.name.as_str() == name)
                .then_some(index)
        })
    }
}
