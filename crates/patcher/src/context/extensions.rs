// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use reseam_apk::reseam_dex::types::header::Loading;
use reseam_apk::reseam_dex::{DexFile, ParseOptions, parse_file};
use tracing::debug;

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
    file: Option<DexFile>,
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
        let options = ParseOptions {
            classes: Loading::Deferred,
            ..ParseOptions::default()
        };
        let dex = parse_file(path, options).map_err(|error| {
            PatcherError::Bundle(format!(
                "failed to parse extension DEX {}: {error}",
                path.display()
            ))
        })?;
        let mut validation = dex.classes().clone();
        for index in (0..validation.len()).rev() {
            drop(validation.remove(index, options).map_err(|error| {
                PatcherError::Bundle(format!(
                    "invalid extension DEX {} class {index}: {error}",
                    path.display()
                ))
            })?);
        }
        let index = self.files.len();
        for header in dex.classes().headers() {
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
        self.files.push(ExtensionDex {
            path: path.to_path_buf(),
            file: Some(dex),
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
            debug!(path = %self.extensions.files[index].path.display(), "linking extension");
            pending.extend(
                dex.types()
                    .iter()
                    .filter_map(|string| {
                        self.extensions
                            .providers
                            .get(dex.string(string).as_ref())
                            .copied()
                    })
                    .filter(|&provider| provider != index),
            );
            self.apk_mut().add_dex(dex);
        }
    }
}
