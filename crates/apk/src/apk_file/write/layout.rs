// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::super::ComponentIndex;
use super::super::component::{ApkComponent, EntryEdit};
use super::{DexJob, ProfileRewrite, SignaturePolicy};
use crate::art_profile;
use crate::entry::{EntryName, is_signature_entry};
use crate::zip::reader;

pub(super) enum EntrySource<'a> {
    Archive(usize),
    Edit(&'a EntryEdit),
    Dex(usize),
}

pub(super) struct PlannedEntry<'a> {
    pub name: EntryName,
    pub source: EntrySource<'a>,
}

/// The entries of one output component in write order: its other entries in
/// archive order, then its rewritten DEX in job order, so copying overlaps DEX
/// serialization. A rewritten baseline `profile` leaves its entries out: they
/// are written after this plan, once every DEX entry and its checksum is known.
pub(super) fn plan<'a>(
    component: &'a ApkComponent,
    index: ComponentIndex,
    jobs: &[DexJob],
    signatures: SignaturePolicy,
    profile: Option<&ProfileRewrite>,
) -> Vec<PlannedEntry<'a>> {
    let dex_jobs: HashMap<_, _> = jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| job.component == index)
        .map(|(job_index, job)| (&job.name, job_index))
        .collect();
    let names = component
        .archive()
        .file_names()
        .map(EntryName::from)
        .chain(
            component
                .edits()
                .iter()
                .filter(|(name, edit)| {
                    !matches!(edit, EntryEdit::Dex | EntryEdit::Deleted)
                        && !dex_jobs.contains_key(name)
                        && !reader::contains(component.archive(), name.as_str())
                })
                .map(|(name, _)| name.clone()),
        )
        .chain(
            jobs.iter()
                .filter(|job| {
                    job.component == index
                        && !reader::contains(component.archive(), job.name.as_str())
                })
                .map(|job| job.name.clone()),
        );
    let mut entries: Vec<_> = names
        .filter_map(|name| {
            if matches!(
                component.edits().get(name.as_str()),
                Some(EntryEdit::Deleted)
            ) || (signatures == SignaturePolicy::Strip && is_signature_entry(name.as_str()))
                || (profile.is_some() && art_profile::ENTRIES.contains(&name.as_str()))
            {
                return None;
            }
            let source = if let Some(&job) = dex_jobs.get(&name) {
                EntrySource::Dex(job)
            } else if let Some(edit) = component.edits().get(name.as_str()) {
                EntrySource::Edit(edit)
            } else {
                EntrySource::Archive(
                    component
                        .archive()
                        .index_for_name(name.as_str())
                        .expect("name came from the archive"),
                )
            };
            Some(PlannedEntry { name, source })
        })
        .collect();
    entries.sort_by_key(|entry| match entry.source {
        EntrySource::Dex(job) => Some(job),
        EntrySource::Archive(_) | EntrySource::Edit(_) => None,
    });
    entries
}
