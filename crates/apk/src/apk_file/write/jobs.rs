// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::super::component::EntryEdit;
use super::super::dex_set::{DexIndex, DexMember, DexSource};
use super::super::{ApkFile, ComponentIndex};
use crate::entry::{EntryName, next_free_dex_name};
use crate::error::Result;
use crate::zip::Compression;

/// One DEX entry to serialize, stored the way its component stores DEX.
pub(in crate::apk_file) struct DexJob {
    pub members: Vec<DexMember>,
    pub component: ComponentIndex,
    pub name: EntryName,
    pub compression: Compression,
}

impl ApkFile {
    /// The DEX entries each component must rewrite, in component order and,
    /// within a component, in archive order followed by new entries.
    pub(super) fn dex_jobs(&self) -> Result<Vec<DexJob>> {
        let rewritten: HashSet<_> = self
            .dex
            .slots()
            .filter(|&(index, _, _)| self.dex.is_rewritten(index))
            .map(|(_, _, origin)| (origin.component, &origin.name))
            .collect();
        let mut used: Vec<HashSet<EntryName>> = self
            .components
            .iter()
            .map(|component| {
                component
                    .reserved_dex_names()
                    .filter(|name| !matches!(component.edits().get(*name), Some(EntryEdit::Dex)))
                    .map(EntryName::from)
                    .collect()
            })
            .collect();
        let mut jobs = Vec::new();
        let mut emitted = HashSet::new();
        for (dex_index, dex, origin) in self.dex.slots() {
            if origin.kind == DexSource::Removed
                || !self.components[origin.component.0].contains(origin.name.as_str())
                || !rewritten.contains(&(origin.component, &origin.name))
            {
                continue;
            }
            let component = origin.component;
            if !emitted.insert((component, &origin.name)) {
                continue;
            }
            let compression = self.components[component.0].dex_compression(origin.name.as_str())?;
            let name = if origin.kind == DexSource::Added {
                next_free_dex_name(&mut used[component.0])
            } else {
                origin.name.clone()
            };
            if dex.header().version.is_container_format() {
                let members = self
                    .dex
                    .slots()
                    .filter(|(_, _, member)| {
                        member.kind != DexSource::Removed
                            && member.component == component
                            && member.name == origin.name
                    })
                    .map(|(index, dex, _)| members_for(index, dex))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                jobs.push(DexJob {
                    members,
                    component,
                    name,
                    compression,
                });
                continue;
            }
            for (ordinal, member) in members_for(dex_index, dex)?.into_iter().enumerate() {
                let name = if ordinal == 0 {
                    name.clone()
                } else {
                    next_free_dex_name(&mut used[component.0])
                };
                jobs.push(DexJob {
                    members: vec![member],
                    component,
                    name,
                    compression,
                });
            }
        }
        Ok(jobs)
    }
}

fn members_for(index: DexIndex, dex: &reseam_dex::DexFile) -> Result<Vec<DexMember>> {
    Ok(match reseam_dex::split_to_fit(dex)? {
        None => vec![DexMember {
            dex_index: index,
            part: None,
        }],
        Some(parts) => parts
            .into_iter()
            .map(|part| DexMember {
                dex_index: index,
                part: Some(part),
            })
            .collect(),
    })
}
