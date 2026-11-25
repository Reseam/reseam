// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_model::Abi;

use super::{ApkComponent, ApkFile};
use crate::entry::native_library;
use crate::error::Result;

const ABI_SPLIT_TYPE_SUFFIX: &str = "__abi";

impl ApkComponent {
    fn native_abis(&self) -> HashSet<Abi> {
        self.entry_names()
            .iter()
            .filter_map(|name| native_library(name).map(|(abi, _)| abi))
            .collect()
    }

    /// A split that holds one module's native libraries for one ABI, as its
    /// `android:splitTypes` declares.
    fn is_abi_split(&self) -> bool {
        self.manifest()
            .split_types()
            .iter()
            .any(|split_type| split_type.ends_with(ABI_SPLIT_TYPE_SUFFIX))
    }
}

/// The ABI Android loads the native libraries of `components`, base first,
/// for on a device whose ABIs are `device`, most preferred first: the first
/// one any component has libraries for. `None` when none of them has any, or
/// when the app loads every ABI the device runs (`android:multiArch`).
fn loaded_abi(components: &[ApkComponent], device: &[Abi]) -> Option<Abi> {
    if components.first()?.manifest().multi_arch() {
        return None;
    }
    let present: HashSet<Abi> = components
        .iter()
        .flat_map(ApkComponent::native_abis)
        .collect();
    device.iter().copied().find(|abi| present.contains(abi))
}

/// The components, base first, that a device whose ABIs are `device` installs:
/// all of them but the ABI splits of ABIs it doesn't load.
pub(super) fn installed_components(
    mut components: Vec<ApkComponent>,
    device: &[Abi],
) -> Vec<ApkComponent> {
    if let Some(abi) = loaded_abi(&components, device) {
        components.retain(|component| {
            !component.is_abi_split() || component.native_abis().contains(&abi)
        });
    }
    components
}

/// A copy of a native library: its component, its ABI and its entry,
/// `lib/<abi>/<name>.so`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLibrary {
    pub component: usize,
    pub abi: Abi,
    pub entry: String,
}

impl ApkFile {
    /// Every copy of the native library named `file`, in each component, for
    /// each ABI Android knows.
    pub fn native_library_copies(&self, file: &str) -> Vec<NativeLibrary> {
        self.components
            .iter()
            .enumerate()
            .flat_map(|(component, apk)| {
                apk.entry_names().into_iter().filter_map(move |entry| {
                    let (abi, name) = native_library(&entry)?;
                    (name == file).then_some(NativeLibrary {
                        component,
                        abi,
                        entry,
                    })
                })
            })
            .collect()
    }

    /// The ABI Android loads this app's native libraries for on the device the
    /// set was opened for. `None` when that device is any, when the app has
    /// libraries for none of its ABIs, or loads every ABI (`android:multiArch`).
    pub fn native_abi(&self) -> Option<Abi> {
        loaded_abi(&self.components, &self.device)
    }

    /// Deletes the native libraries of every ABI but `abi`.
    pub fn retain_native_abi(&mut self, abi: Abi) -> Result<()> {
        for index in 0..self.components.len() {
            for name in self.components[index].entry_names() {
                if native_library(&name).is_some_and(|(library, _)| library != abi) {
                    self.delete_file(index, &name)?;
                }
            }
        }
        Ok(())
    }
}
