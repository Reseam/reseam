// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use reseam_dex::Loading;
use reseam_dex::write_class;
use reseam_dex::{DexFile, ParseOptions, parse_file};
use tracing::debug;

use super::PatchContext;
use crate::error::{ExtensionError, PatcherError, Result};

fn options() -> ParseOptions {
    ParseOptions {
        classes: Loading::Deferred,
        ..ParseOptions::default()
    }
}

pub struct ExtensionSet {
    files: Vec<ExtensionDex>,
    providers: HashMap<String, usize>,
    warned: HashSet<String>,
}

struct ExtensionDex {
    source: Source,
    file: Option<DexFile>,
}

enum Source {
    AppRuntime,
    Bundle(PathBuf),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppRuntime => f.write_str("the app runtime"),
            Self::Bundle(path) => path.display().fmt(f),
        }
    }
}

impl ExtensionSet {
    /// The app runtime the engine embeds, which every patch run can link.
    pub fn app_runtime() -> Self {
        #[cfg_attr(
            not(feature = "bridge"),
            expect(unused_mut, reason = "only the Kotlin bridge links the app runtime")
        )]
        let mut set = Self {
            files: Vec::new(),
            providers: HashMap::new(),
            warned: HashSet::new(),
        };
        #[cfg(feature = "bridge")]
        set.add(
            Source::AppRuntime,
            reseam_dex::parse(crate::app_runtime::DEX, options()),
        )
        .expect("the build embeds the app runtime as valid DEX");
        set
    }

    /// The app runtime, then the bundles' extension files.
    pub fn load(paths: &[PathBuf]) -> Result<Self> {
        let mut set = Self::app_runtime();
        for path in paths {
            set.add(Source::Bundle(path.clone()), parse_file(path, options()))?;
        }
        Ok(set)
    }

    fn add(&mut self, source: Source, parsed: reseam_dex::Result<DexFile>) -> Result<()> {
        let failed = |reason| PatcherError::Extension {
            file: source.to_string(),
            reason,
        };
        let mut dex = parsed.map_err(|error| failed(error.into()))?;
        let mut validation = dex.classes().clone();
        for index in (0..validation.len()).rev() {
            drop(
                validation
                    .remove(index, options())
                    .map_err(|source| failed(ExtensionError::Class { index, source }))?,
            );
        }
        let index = self.files.len();
        let mut shared = Vec::new();
        for (class_idx, header) in dex.classes().headers().enumerate() {
            let descriptor = dex.type_descriptor(header.class_type).into_owned();
            if matches!(source, Source::Bundle(_))
                && descriptor.starts_with(crate::app_runtime::PACKAGE)
            {
                return Err(failed(ExtensionError::RuntimeClass(descriptor)));
            }
            match self.providers.get(&descriptor) {
                None => {
                    self.providers.insert(descriptor, index);
                }
                Some(&other)
                    if self.defines_same(other, &descriptor, &dex, class_idx, &source)? =>
                {
                    shared.push(header.class_type);
                }
                Some(&other) => {
                    return Err(failed(ExtensionError::Conflict {
                        descriptor,
                        other: self.files[other].source.to_string(),
                    }));
                }
            }
        }
        // Bundles built apart each ship their own copy of shared classes, such as the
        // java.lang.Record d8 synthesizes; one copy serves every extension that uses it.
        for class_type in shared {
            dex.remove_class(class_type)
                .map_err(|error| failed(error.into()))?;
        }
        self.files.push(ExtensionDex {
            source,
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
        source: &Source,
    ) -> Result<bool> {
        let provider = &self.files[other];
        let provider_dex = provider
            .file
            .as_ref()
            .expect("extensions are linked only after every bundle is loaded");
        let provider_idx = provider_dex
            .find_class_index(descriptor)
            .expect("a provider defines the classes it is registered for");
        let image = |dex: &DexFile, class_idx: usize, source: &Source| {
            write_class(dex, class_idx).map_err(|error| PatcherError::Extension {
                file: source.to_string(),
                reason: error.into(),
            })
        };
        Ok(image(provider_dex, provider_idx, &provider.source)? == image(dex, class_idx, source)?)
    }

    fn provider(&self, descriptor: &str) -> Option<usize> {
        self.providers.get(descriptor).copied()
    }

    /// The definition of `descriptor` in an extension not linked yet.
    pub(super) fn unlinked_class(&self, descriptor: &str) -> Option<(&DexFile, usize)> {
        let dex = self.files[self.provider(descriptor)?].file.as_ref()?;
        Some((dex, dex.find_class_index(descriptor)?))
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
    /// Makes sure every class the descriptors refer to is defined, merging
    /// extension DEX files as needed. A reference nothing defines is logged
    /// once, since it is almost always a typo in the patch.
    pub fn link_types<'s>(&mut self, descriptors: impl IntoIterator<Item = &'s str>) {
        for descriptor in descriptors {
            let Some(class) = class_of(descriptor) else {
                continue;
            };
            if is_platform(class) || self.find_class(class).is_some() {
                continue;
            }
            match self.extensions.provider(class) {
                Some(index) => self.merge_extension(index),
                None => {
                    if self.extensions.warned.insert(class.to_owned()) {
                        self.journal
                            .record(super::undo::Part::Warned(class.to_owned()), || {
                                super::undo::Undo::Warned(class.to_owned())
                            });
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
            debug!(source = %self.extensions.files[index].source, "linking extension");
            pending.extend(
                dex.types()
                    .iter()
                    .filter_map(|string| {
                        self.extensions
                            .providers
                            .get(dex.strings().get(string).as_ref())
                            .copied()
                    })
                    .filter(|&provider| provider != index),
            );
            let dex_index = self.apk.dex().len();
            self.journal.record(super::undo::Part::Merge(index), || {
                super::undo::Undo::Merged {
                    extension: index,
                    dex: dex_index,
                }
            });
            self.apk_mut().add_dex(dex);
        }
    }
}

impl ExtensionSet {
    /// Takes back extension `index`, which a rollback removed from the app.
    pub(super) fn restore(&mut self, index: usize, dex: DexFile) {
        self.files[index].file = Some(dex);
    }

    pub(super) fn forget_warning(&mut self, class: &str) {
        self.warned.remove(class);
    }
}
