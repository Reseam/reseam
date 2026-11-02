// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod catalog;
mod code_map;
mod runtime;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use reseam_apk::AxmlDocument;
use reseam_apk::axml::android_attrs::ATTR_NAME;
use reseam_apk::axml::children;
use reseam_model::analysis::{
    ClassRef, ComponentKind, DexLanguage, EntryPoints, Marker, Overview, SdkUse,
};

use super::Analysis;
use super::manifest::{application, components};
use crate::error::Result;
use catalog::{KOTLIN_TYPES, PROTECTIONS, SDKS};

impl Analysis {
    /// What runs the app, where its code is, which SDKs it uses and where it starts.
    pub fn overview(&mut self) -> Result<Overview> {
        let entries: Vec<String> = self
            .opened
            .apk
            .components()
            .iter()
            .flat_map(reseam_apk::ApkComponent::entry_names)
            .collect();
        let bundle_header = self.entry_prefix(runtime::JS_BUNDLE, 12)?;
        let manifest = self.opened.apk.base().manifest();
        let classes: Vec<&ClassRef> = self.hierarchy().classes().map(|(class, _)| class).collect();
        let packages = package_counts(&classes);
        let package = self
            .opened
            .apk
            .package_name()
            .map(std::borrow::Cow::into_owned);
        Ok(Overview {
            apk: self.apk()?,
            runtimes: runtime::detect(&entries, bundle_header.as_deref()),
            dex_language: self.dex_language(),
            markers: markers(&packages, &entries),
            code: code_map::code_map(classes.iter().copied(), package.as_deref()),
            sdks: sdks(&packages, &manifest_names(manifest)),
            entry: entry_points(manifest),
        })
    }

    /// Kotlin when the DEX names a type of Kotlin's runtime library.
    fn dex_language(&self) -> DexLanguage {
        if self.dex().iter().any(|dex| {
            KOTLIN_TYPES
                .iter()
                .any(|type_| dex.find_type_idx(type_).is_some())
        }) {
            DexLanguage::Kotlin
        } else {
            DexLanguage::Java
        }
    }

    /// Up to `len` leading bytes of entry `name` in the first component that has it.
    fn entry_prefix(&mut self, name: &str, len: u64) -> Result<Option<Vec<u8>>> {
        let apk = &mut self.opened.apk;
        let Some(component) = apk
            .components()
            .iter()
            .position(|component| component.contains(name))
        else {
            return Ok(None);
        };
        let reader = apk
            .open_component_entry(component, name)
            .map_err(|error| self.opened.error(error))?
            .expect("the component was found holding the entry");
        let mut prefix = Vec::new();
        reader
            .take(len)
            .read_to_end(&mut prefix)
            .map_err(|error| self.opened.error(error.into()))?;
        Ok(Some(prefix))
    }
}

/// How many classes each package holds directly.
fn package_counts(classes: &[&ClassRef]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for class in classes {
        *counts.entry(class.package()).or_default() += 1;
    }
    counts
}

/// How many classes `prefix` and its subpackages hold.
fn classes_in(packages: &BTreeMap<String, usize>, prefix: &str) -> usize {
    packages
        .range(prefix.to_owned()..)
        .take_while(|(package, _)| package.starts_with(prefix))
        .filter(|(package, _)| in_package(package, prefix))
        .map(|(_, count)| count)
        .sum()
}

fn markers(packages: &BTreeMap<String, usize>, entries: &[String]) -> Vec<Marker> {
    PROTECTIONS
        .iter()
        .filter_map(|protection| {
            let mut evidence: BTreeSet<String> = BTreeSet::new();
            for package in protection.packages {
                let count = classes_in(packages, package);
                if count > 0 {
                    evidence.insert(format!("{count} classes in {package}"));
                }
            }
            evidence.extend(
                entries
                    .iter()
                    .filter(|entry| {
                        protection
                            .libraries
                            .iter()
                            .any(|library| entry.rsplit('/').next() == Some(library))
                    })
                    .cloned(),
            );
            (!evidence.is_empty()).then(|| Marker {
                name: protection.name.to_owned(),
                evidence: evidence.into_iter().collect(),
            })
        })
        .collect()
}

/// Permission and meta-data names the manifest lists, and its component classes.
fn manifest_names(manifest: &AxmlDocument) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let events = manifest.events();
    let mut elements: Vec<usize> = manifest.root().into_iter().collect();
    while let Some(element) = elements.pop() {
        elements.extend(children(events, element));
        if let Some(name) = manifest
            .attribute(element, ATTR_NAME)
            .and_then(|name| manifest.attribute_string(name))
        {
            names.insert(name.into_owned());
        }
    }
    names
}

fn sdks(packages: &BTreeMap<String, usize>, manifest: &BTreeSet<String>) -> Vec<SdkUse> {
    let mut found: Vec<SdkUse> = SDKS
        .iter()
        .filter_map(|sdk| {
            let count = classes_in(packages, sdk.package);
            let mut evidence: Vec<String> = sdk
                .manifest
                .iter()
                .filter(|name| manifest.contains(**name))
                .map(|name| format!("manifest {name}"))
                .collect();
            if count > 0 {
                evidence.insert(0, format!("{count} classes in {}", sdk.package));
            }
            (!evidence.is_empty()).then(|| SdkUse {
                category: sdk.category,
                name: sdk.name.to_owned(),
                package: sdk.package.to_owned(),
                classes: count,
                evidence,
            })
        })
        .collect();
    found.sort_by(|a, b| (a.category, &a.name).cmp(&(b.category, &b.name)));
    found
}

fn entry_points(manifest: &AxmlDocument) -> EntryPoints {
    let components = components(manifest);
    let count = |kinds: &[ComponentKind]| {
        components
            .iter()
            .filter(|component| kinds.contains(&component.kind))
            .count()
    };
    EntryPoints {
        application: application(manifest),
        launcher: components
            .iter()
            .find(|component| component.launcher)
            .map(|component| component.class.clone()),
        activities: count(&[ComponentKind::Activity, ComponentKind::ActivityAlias]),
        services: count(&[ComponentKind::Service]),
        receivers: count(&[ComponentKind::Receiver]),
        providers: count(&[ComponentKind::Provider]),
    }
}

fn in_package(package: &str, prefix: &str) -> bool {
    package == prefix
        || package
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}
