// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{DexFile, MethodIdx, TypeIdx, WriteLayout};
use rustc_hash::FxHashMap;

use super::{DexMetadata, DexProfile};

impl DexProfile {
    /// The entries of this profile, recorded against `dex`, whose classes
    /// `layout` placed in the DEX written as `name`, in that DEX's indices.
    /// Classes removed from `dex` drop out with their methods.
    pub(crate) fn rewrite(
        &self,
        dex: &DexFile,
        layout: &WriteLayout,
        name: &str,
        checksum: u32,
    ) -> Self {
        let class_defs: FxHashMap<TypeIdx, u32> = layout
            .class_types()
            .iter()
            .zip(0..)
            .map(|(&class, def)| (class, def))
            .collect();
        let (mut classes, mut defs): (Vec<u32>, Vec<u32>) = self
            .classes
            .iter()
            .filter_map(|&class| {
                let class = TypeIdx(class);
                Some((layout.type_index(class)?.0, *class_defs.get(&class)?))
            })
            .unzip();
        classes.sort_unstable();
        defs.sort_unstable();
        let methods = self
            .methods
            .iter()
            .filter_map(|(&method, &flags)| {
                let declaring = dex.methods().try_get(method as usize)?.class;
                class_defs.contains_key(&declaring).then_some(())?;
                Some((layout.method_index(MethodIdx(method))?.0, flags))
            })
            .collect();
        Self {
            name: name.to_owned(),
            checksum,
            method_count: layout.method_count(),
            classes,
            methods,
            metadata: Some(DexMetadata {
                type_count: layout.type_count(),
                class_defs: defs,
            }),
        }
    }
}
