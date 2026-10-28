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
    let archived = component
        .archive_entries()
        .iter()
        .enumerate()
        .map(|(archive_index, name)| {
            let source = match (dex_jobs.get(name), component.edits().get(name.as_str())) {
                (Some(&job), _) => EntrySource::Dex(job),
                (None, Some(edit)) => EntrySource::Edit(edit),
                (None, None) => EntrySource::Archive(archive_index),
            };
            (name.clone(), source)
        });
    let added_files = component
        .edits()
        .iter()
        .filter(|(name, edit)| {
            !matches!(edit, EntryEdit::Dex | EntryEdit::Deleted)
                && !dex_jobs.contains_key(name)
                && !reader::contains(component.archive(), name.as_str())
        })
        .map(|(name, edit)| (name.clone(), EntrySource::Edit(edit)));
    let added_dex = jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| {
            job.component == index && !reader::contains(component.archive(), job.name.as_str())
        })
        .map(|(job_index, job)| (job.name.clone(), EntrySource::Dex(job_index)));
    let mut entries: Vec<_> = archived
        .chain(added_files)
        .chain(added_dex)
        .filter(|(name, _)| {
            let name = name.as_str();
            let deleted = matches!(component.edits().get(name), Some(EntryEdit::Deleted));
            let stripped = signatures == SignaturePolicy::Strip && is_signature_entry(name);
            let profiled = profile.is_some() && art_profile::ENTRIES.contains(&name);
            !(deleted || stripped || profiled)
        })
        .map(|(name, source)| PlannedEntry { name, source })
        .collect();
    entries.sort_by_key(|entry| match entry.source {
        EntrySource::Dex(job) => Some(job),
        EntrySource::Archive(_) | EntrySource::Edit(_) => None,
    });
    entries
}
