// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Classes that would be written to DEX files the app did not come with, moved
//! into the ones it did. Some apps count their DEX files, and every added one
//! is another file Android opens at start.

use std::cmp::Reverse;

use rayon::prelude::*;
use reseam_dex::{ClassImport, DexFile, ImportedPools, split_to_fit};

use super::dex_set::{DexIndex, DexSource};
use super::undo::{Part, Undo};
use super::{ApkFile, ComponentIndex};
use crate::error::Result;

impl ApkFile {
    /// Asks for [`Self::fold_dex`] once editing is over. Patches ask for it,
    /// since only some apps mind the DEX files patching adds.
    pub fn request_dex_folding(&mut self) {
        let requested = self.folds_dex;
        self.journal
            .record(Part::DexFolding, || Undo::DexFolding(requested));
        self.folds_dex = true;
    }

    pub fn dex_folding_requested(&self) -> bool {
        self.folds_dex
    }

    /// Moves classes into the DEX files of their APK that have room for their
    /// ids: every class of a DEX the session added, and the classes an edited
    /// DEX no longer has room for. Edited DEX files take classes first, since
    /// they are serialized again anyway, then the roomiest. A class that fits
    /// none stays where it is and is written to a DEX file of its own, as an
    /// added DEX with classes left is; one left without classes is not written.
    ///
    /// Classes move, so call this once editing is over.
    pub fn fold_dex(&mut self) -> Result<()> {
        for component in (0..self.components.len()).map(ComponentIndex) {
            let slots: Vec<_> = self
                .dex
                .slots()
                .filter(|(_, _, origin)| origin.component == component)
                .collect();
            let mut hosts: Vec<DexIndex> = slots
                .iter()
                .filter(|(_, dex, origin)| {
                    origin.kind == DexSource::Archive && dex.id_headroom() > 0
                })
                .map(|&(index, _, _)| index)
                .collect();
            let sources = slots
                .into_par_iter()
                .filter_map(|(index, dex, origin)| {
                    let classes = match origin.kind {
                        DexSource::Added => Ok((0..dex.classes().len()).collect()),
                        DexSource::Archive if dex.id_headroom() == 0 => spilled(dex),
                        DexSource::Archive | DexSource::Removed => return None,
                    };
                    Some(classes.map(|classes| (index, classes)))
                })
                .collect::<Result<Vec<_>>>()?;
            hosts.sort_by_key(|&host| {
                let dex = self.dex.dex(host);
                (!dex.is_dirty(), Reverse(dex.id_headroom()))
            });
            for (source, classes) in sources {
                self.move_classes(source, &classes, &mut hosts)?;
            }
        }
        Ok(())
    }

    /// Moves `classes` of DEX `source` into the first of `hosts` that takes
    /// each, dropping the hosts that are full.
    fn move_classes(
        &mut self,
        source: DexIndex,
        classes: &[usize],
        hosts: &mut Vec<DexIndex>,
    ) -> Result<()> {
        let mut moved = Vec::new();
        let mut pools = ImportedPools::none(self.dex.dex(source));
        for &class_idx in classes {
            while let Some(&host) = hosts.first() {
                let (source, host) = self.dex.pair_mut(source, host);
                match host.import_class(source, class_idx, &mut pools)? {
                    ClassImport::Imported(_) => {
                        moved.push(source.classes().header(class_idx).class_type);
                        break;
                    }
                    ClassImport::Full => {
                        hosts.remove(0);
                        pools = ImportedPools::none(source);
                    }
                    ClassImport::Unspellable => break,
                }
            }
        }
        let origin = self.dex.origin(source);
        if origin.kind == DexSource::Added && moved.len() == self.dex.dex(source).classes().len() {
            let (component, name) = (origin.component, origin.name.as_str().to_owned());
            return self.delete_file(component.0, &name);
        }
        let source = self
            .dex
            .live_mut(source.0)
            .expect("a DEX that classes were moved from is live");
        for class_type in moved {
            source.remove_class(class_type)?;
        }
        Ok(())
    }
}

/// The classes `dex` has no room for: those of every part but the first that
/// writing it would split it into.
fn spilled(dex: &DexFile) -> Result<Vec<usize>> {
    Ok(split_to_fit(dex)?
        .iter()
        .flatten()
        .skip(1)
        .flat_map(|part| part.classes().iter().copied())
        .collect())
}
