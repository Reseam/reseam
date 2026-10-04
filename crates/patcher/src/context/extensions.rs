// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use reseam_apk::reseam_dex::types::header::Loading;
use reseam_apk::reseam_dex::write::write_class;
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
        let mut dex = parse_file(path, options).map_err(|error| {
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
        let mut shared = Vec::new();
        for (class_idx, header) in dex.classes().headers().enumerate() {
            let descriptor = dex.type_descriptor(header.class_type).into_owned();
            match self.providers.get(&descriptor) {
                None => {
                    self.providers.insert(descriptor, index);
                }
                Some(&other) if self.defines_same(other, &descriptor, &dex, class_idx, path)? => {
                    shared.push(header.class_type);
                }
                Some(&other) => {
                    return Err(PatcherError::Bundle(format!(
                        "class {descriptor} is defined differently by {} and {}",
                        self.files[other].path.display(),
                        path.display()
                    )));
                }
            }
        }
        // Bundles built apart each ship their own copy of shared classes, such as the
        // java.lang.Record d8 synthesizes; one copy serves every extension that uses it.
        for class_type in shared {
            dex.remove_class(class_type).map_err(|error| {
                PatcherError::Bundle(format!("extension DEX {}: {error}", path.display()))
            })?;
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

    /// Whether extension `other` defines `descriptor` exactly as class `class_idx` of `dex`.
    fn defines_same(
        &self,
        other: usize,
        descriptor: &str,
        dex: &DexFile,
        class_idx: usize,
        path: &Path,
    ) -> Result<bool> {
        let provider = &self.files[other];
        let provider_dex = provider
            .file
            .as_ref()
            .expect("extensions are linked only after every bundle is loaded");
        let provider_idx = provider_dex
            .find_class_index(descriptor)
            .expect("a provider defines the classes it is registered for");
        let image = |dex: &DexFile, class_idx: usize, path: &Path| {
            write_class(dex, class_idx).map_err(|error| {
                PatcherError::Bundle(format!(
                    "failed to write {descriptor} from {}: {error}",
                    path.display()
                ))
            })
        };
        Ok(image(provider_dex, provider_idx, &provider.path)? == image(dex, class_idx, path)?)
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
