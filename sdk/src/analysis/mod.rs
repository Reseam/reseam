// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Read-only analysis of one APK: where its code is, what references what,
//! and its methods, classes, components and resources as text. Nothing here
//! changes the APK.

mod calls;
mod code;
mod diff;
mod hierarchy;
mod hints;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
mod inspect;
mod manifest;
mod named;
mod overview;
mod refs;
mod resources;
mod show;
mod text;
mod xml;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use reseam_apk::DexSet;
use reseam_apk::resources::StringEntries;
use reseam_dex::{ClassDef, DexFile, MethodIdx};
use reseam_model::analysis::{ApkView, ClassRef, Member, MethodRef};
use sha2::{Digest, Sha256};

use crate::apk::OpenedApk;
use crate::error::{HostError, Result};
pub use diff::diff;
use hierarchy::Hierarchy;
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub use reseam_patcher::inspect::KOTLIN_VERSION;
use xml::XmlIndex;

/// An APK opened for analysis, with the indexes analysis builds on first use.
pub struct Analysis {
    opened: OpenedApk,
    hierarchy: OnceLock<Hierarchy>,
    /// The string index of each resource table, in the order of [`Self::tables`].
    string_entries: OnceLock<Vec<StringEntries>>,
    xml: Option<XmlIndex>,
}

impl Analysis {
    /// Opens an APK, its splits, or an APKM or XAPK container, as patching does.
    pub fn open(path: &Path, split_paths: &[PathBuf]) -> Result<Self> {
        let mut opened = OpenedApk::open(path, split_paths)?;
        for index in 0..opened.apk.components().len() {
            let component = opened
                .apk
                .component_mut(index)
                .expect("the index is within the components");
            if let Err(error) = component.resources() {
                return Err(opened.error(error));
            }
        }
        Ok(Self {
            opened,
            hierarchy: OnceLock::new(),
            string_entries: OnceLock::new(),
            xml: None,
        })
    }

    /// The APK as a file: identity, splits, DEX sizes and signers.
    pub fn apk(&self) -> Result<ApkView> {
        let apk = &self.opened.apk;
        let source = apk
            .base()
            .source()
            .map_err(|error| self.opened.error(error))?;
        let signers = reseam_sign::signer_certificates(&source)?
            .iter()
            .map(|certificate| hex::encode(Sha256::digest(certificate)))
            .collect();
        Ok(ApkView {
            package: apk.package_name().map(Cow::into_owned),
            version_name: apk.version_name().map(Cow::into_owned),
            version_code: apk.version_code(),
            min_sdk: apk.base().manifest().min_sdk_version(),
            splits: apk.components()[1..]
                .iter()
                .map(|component| component.name().to_string())
                .collect(),
            dex_files: apk.dex().len(),
            classes: apk.dex().iter().map(|dex| dex.classes().len()).sum(),
            methods: apk.dex().iter().map(|dex| dex.methods().len()).sum(),
            signers,
        })
    }

    fn dex(&self) -> &DexSet {
        self.opened.apk.dex()
    }

    fn hierarchy(&self) -> &Hierarchy {
        self.hierarchy.get_or_init(|| Hierarchy::build(self.dex()))
    }

    /// The DEX defining `class`, and the class.
    fn class(&self, class: &ClassRef) -> Result<(&DexFile, Cow<'_, ClassDef>)> {
        let info = self
            .hierarchy()
            .get(class)
            .ok_or_else(|| HostError::UnknownMember(Member::Class(class.clone())))?;
        let dex = self
            .dex()
            .get(info.dex)
            .expect("the hierarchy holds positions of this set");
        Ok((dex, dex.classes().read(info.index, dex.parse_options())?))
    }

    /// The DEX defining `method`, its class, and its id in that DEX.
    fn method(&self, method: &MethodRef) -> Result<(&DexFile, Cow<'_, ClassDef>, MethodIdx)> {
        let unknown = || HostError::UnknownMember(Member::Method(method.clone()));
        let (dex, class) = self.class(&method.class)?;
        let idx = method_idx(dex, method).ok_or_else(unknown)?;
        code::encoded_method(&class, idx).ok_or_else(unknown)?;
        Ok((dex, class, idx))
    }
}

/// The id `dex` gives `method`, when its pools name it.
fn method_idx(dex: &DexFile, method: &MethodRef) -> Option<MethodIdx> {
    let class = dex.find_type_idx(method.class.descriptor())?;
    dex.find_method(class, &method.name, &method.proto)
}
