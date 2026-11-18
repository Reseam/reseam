// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod link;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use rayon::prelude::*;
use reseam_dex::Loading;
use reseam_dex::write_class;
use reseam_dex::{DexFile, ParseOptions, parse_file};

use crate::error::{ExtensionError, PatcherError, Result};

fn options() -> ParseOptions {
    ParseOptions {
        classes: Loading::Deferred,
        ..ParseOptions::default()
    }
}

pub struct ExtensionSet {
    pub(super) files: Vec<ExtensionDex>,
    pub(super) providers: HashMap<String, usize>,
    pub(super) warned: HashSet<String>,
}

pub(super) struct ExtensionDex {
    pub(super) source: Source,
    pub(super) file: Option<DexFile>,
}

pub(super) enum Source {
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
        let parsed: Vec<(Source, DexFile)> = paths
            .par_iter()
            .map(|path| {
                let source = Source::Bundle(path.clone());
                let dex = parse_and_validate(&source, parse_file(path, options()))?;
                Ok((source, dex))
            })
            .collect::<Result<_>>()?;
        for (source, dex) in parsed {
            set.register(source, dex)?;
        }
        Ok(set)
    }

    fn add(&mut self, source: Source, parsed: reseam_dex::Result<DexFile>) -> Result<()> {
        let dex = match source {
            Source::AppRuntime => parsed.map_err(|error| PatcherError::Extension {
                file: source.to_string(),
                reason: error.into(),
            })?,
            Source::Bundle(_) => parse_and_validate(&source, parsed)?,
        };
        self.register(source, dex)
    }

    fn register(&mut self, source: Source, mut dex: DexFile) -> Result<()> {
        let failed = |reason| PatcherError::Extension {
            file: source.to_string(),
            reason,
        };
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

    pub(super) fn provider(&self, descriptor: &str) -> Option<usize> {
        self.providers.get(descriptor).copied()
    }

    /// The definition of `descriptor` in an extension not linked yet.
    pub(in crate::context) fn unlinked_class(&self, descriptor: &str) -> Option<(&DexFile, usize)> {
        let dex = self.files[self.provider(descriptor)?].file.as_ref()?;
        Some((dex, dex.find_class_index(descriptor)?))
    }

    /// Takes back extension `index`, which a rollback removed from the app.
    pub(in crate::context) fn restore(&mut self, index: usize, dex: DexFile) {
        self.files[index].file = Some(dex);
    }

    pub(in crate::context) fn forget_warning(&mut self, class: &str) {
        self.warned.remove(class);
    }
}

fn parse_and_validate(source: &Source, parsed: reseam_dex::Result<DexFile>) -> Result<DexFile> {
    let failed = |reason| PatcherError::Extension {
        file: source.to_string(),
        reason,
    };
    let dex = parsed.map_err(|error| failed(error.into()))?;
    let mut validation = dex.classes().clone();
    for index in (0..validation.len()).rev() {
        drop(
            validation
                .remove(index, options())
                .map_err(|source| failed(ExtensionError::Class { index, source }))?,
        );
    }
    Ok(dex)
}
