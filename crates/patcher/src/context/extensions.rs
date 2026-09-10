// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extension DEX files a bundle ships, linked into the app on demand: the
//! first reference a patch makes to a class one of them defines merges that
//! file, plus every other extension file it references, into the APK.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use reseam_apk::reseam_dex::{parse_owned, DexFile, ParseOptions};
use tracing::info;

use super::PatchContext;
use crate::error::{PatcherError, Result};

#[derive(Default)]
pub struct ExtensionSet {
    files: Vec<ExtensionDex>,
    providers: HashMap<String, usize>,
    warned: HashSet<String>,
}

struct ExtensionDex {
    path: PathBuf,
    /// Taken when the file is merged into the app.
    file: Option<DexFile>,
    /// Every type the file refers to; the ones other extensions define are
    /// merged along with it.
    references: Vec<String>,
}

impl ExtensionSet {
    pub fn load(paths: &[PathBuf]) -> Result<Self> {
        let mut set = Self::default();
        for path in paths {
            set.add(path)?;
        }
        Ok(set)
    }

    fn add(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path).map_err(|e| {
            PatcherError::Bundle(format!(
                "failed to read extension DEX {}: {e}",
                path.display()
            ))
        })?;
        let dex = parse_owned(bytes, ParseOptions::default()).map_err(|e| {
            PatcherError::Bundle(format!(
                "failed to parse extension DEX {}: {e}",
                path.display()
            ))
        })?;
        let index = self.files.len();
        for header in dex.classes.headers() {
            let descriptor = dex.type_descriptor(header.class_type).into_owned();
            if let Some(&other) = self.providers.get(&descriptor) {
                return Err(PatcherError::Bundle(format!(
                    "class {descriptor} is defined by both {} and {}",
                    self.files[other].path.display(),
                    path.display()
                )));
            }
            self.providers.insert(descriptor, index);
        }
        let references = dex
            .types
            .iter()
            .map(|string| dex.string(string).into_owned())
            .collect();
        self.files.push(ExtensionDex {
            path: path.to_path_buf(),
            file: Some(dex),
            references,
        });
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn provider(&self, descriptor: &str) -> Option<usize> {
        self.providers.get(descriptor).copied()
    }
}

/// The class a reference resolves to: arrays refer to their element type,
/// primitives to nothing.
fn class_of(descriptor: &str) -> Option<&str> {
    let element = descriptor.trim_start_matches('[');
    element.starts_with('L').then_some(element)
}

fn is_platform(descriptor: &str) -> bool {
    const PLATFORM: [&str; 10] = [
        "Ljava/",
        "Ljavax/",
        "Landroid/",
        "Ldalvik/",
        "Llibcore/",
        "Lsun/",
        "Lorg/json/",
        "Lorg/xml/",
        "Lorg/w3c/",
        "Lorg/apache/http/",
    ];
    PLATFORM.iter().any(|prefix| descriptor.starts_with(prefix))
}

impl PatchContext<'_> {
    pub fn set_extensions(&mut self, extensions: ExtensionSet) {
        self.extensions = extensions;
    }

    /// Makes sure every class the descriptors refer to is defined, merging
    /// extension DEX files as needed. A reference nothing defines is logged
    /// once, since it is almost always a typo in the patch.
    pub fn link_types<'s>(&mut self, descriptors: impl IntoIterator<Item = &'s str>) {
        for descriptor in descriptors {
            let Some(class) = class_of(descriptor) else {
                continue;
            };
            if self.find_class(class).is_some() || is_platform(class) {
                continue;
            }
            match self.extensions.provider(class) {
                Some(index) => self.merge_extension(index),
                None => {
                    if self.extensions.warned.insert(class.to_owned()) {
                        self.log().warn(format!(
                            "{class} is not defined by the app or any extension in the bundle"
                        ));
                    }
                }
            }
        }
    }

    /// `find_class` that first links the extension defining `descriptor`.
    pub fn find_or_link_class(&mut self, descriptor: &str) -> Option<super::ClassLocation> {
        if let Some(location) = self.find_class(descriptor) {
            return Some(location);
        }
        let index = self.extensions.provider(descriptor)?;
        self.merge_extension(index);
        self.find_class(descriptor)
    }

    fn merge_extension(&mut self, index: usize) {
        let mut pending = vec![index];
        while let Some(index) = pending.pop() {
            let Some(dex) = self.extensions.files[index].file.take() else {
                continue;
            };
            info!(path = %self.extensions.files[index].path.display(), "linking extension");
            self.apk_mut().add_dex(dex);
            let providers = &self.extensions.providers;
            pending.extend(
                self.extensions.files[index]
                    .references
                    .iter()
                    .filter_map(|reference| providers.get(reference.as_str()).copied())
                    .filter(|&provider| provider != index),
            );
        }
    }
}
