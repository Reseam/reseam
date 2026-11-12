// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which apps a patches release updates for people who patched with the release
//! before it: the apps of the patches whose fingerprints changed, that are new, or
//! that are gone.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io;

use anyhow::{Context, Result, bail};
use reseam_model::{Compatibility, InspectRequest, PatchMetadata, Problem, ReleaseIndex, Trust};
use reseam_sdk::inspect;
use url::Url;

use super::index::{self, unprefixed};
use super::published::{Expected, check, open};

/// The app a patch is for: a package, or any app for a universal patch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum App {
    Package(String),
    Any,
}

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(package) => f.write_str(package),
            Self::Any => f.write_str("any app"),
        }
    }
}

/// How many of an app's patches a release changes, adds and removes.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Updates {
    pub(super) changed: usize,
    pub(super) added: usize,
    pub(super) removed: usize,
}

/// The newest release before this one, as this engine reads it.
enum Previous {
    Patches(Vec<PatchMetadata>),
    /// Built for an engine line this one no longer loads, so nothing in it can be compared and
    /// every app patched with it has to update.
    OtherLine {
        engine: String,
    },
}

/// Prints which apps `patches` update, compared with the newest release `previous` lists.
pub(super) fn report(previous: &ReleaseIndex, patches: &[PatchMetadata]) -> Result<()> {
    let Some(release) = previous.releases.first() else {
        return Ok(());
    };
    let before = match read(previous, &release.download_url, &release.version)? {
        Previous::Patches(before) => before,
        Previous::OtherLine { engine } => {
            println!(
                "Compared with {}, built for engine {engine}, which this engine no longer loads, this release updates every app.",
                release.version
            );
            return Ok(());
        }
    };
    let updates = updates(&before, patches);
    if updates.is_empty() {
        println!(
            "Compared with {}, this release updates no app.",
            release.version
        );
        return Ok(());
    }
    println!("Compared with {}, this release updates:", release.version);
    for (
        app,
        Updates {
            changed,
            added,
            removed,
        },
    ) in updates
    {
        println!("  {app}: {changed} changed, {added} new, {removed} removed");
    }
    Ok(())
}

/// The release at `url`, checked against the index that lists it when this engine can read it.
fn read(previous: &ReleaseIndex, url: &str, version: &str) -> Result<Previous> {
    let signer =
        index::public_key(previous).context("the previous index names no bundle public key")?;
    let url =
        Url::parse(url).with_context(|| format!("the previous release's download URL {url}"))?;
    let mut file = tempfile::NamedTempFile::new()?;
    io::copy(&mut open(&url)?, &mut file).with_context(|| format!("downloading {url}"))?;
    let mut response = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        device_abis: Vec::new(),
        bundle_paths: vec![file.path().display().to_string()],
        trust: Trust {
            keys: vec![signer.clone()],
        },
    })?;
    let bundle = response.bundles.remove(0);
    match bundle.problem {
        Some(Problem::BundleTooOld { built, .. }) => Ok(Previous::OtherLine { engine: built }),
        Some(problem) => bail!("failed to open the previous release: {problem}"),
        None => {
            check(
                &bundle,
                &Expected {
                    name: &previous.bundle.name,
                    version: unprefixed(version),
                    signer,
                },
            )?;
            Ok(Previous::Patches(response.patches))
        }
    }
}

/// What `after` does to each app's patches compared with `before`, for the apps it changes.
pub(super) fn updates(before: &[PatchMetadata], after: &[PatchMetadata]) -> BTreeMap<App, Updates> {
    let old: HashMap<&str, &PatchMetadata> = before
        .iter()
        .map(|patch| (patch.spec.id.as_str(), patch))
        .collect();
    let new: HashMap<&str, &PatchMetadata> = after
        .iter()
        .map(|patch| (patch.spec.id.as_str(), patch))
        .collect();
    let mut updates: BTreeMap<App, Updates> = BTreeMap::new();
    let mut count = |patch: &PatchMetadata, field: fn(&mut Updates) -> &mut usize| {
        for app in apps(&patch.spec.compatibility) {
            *field(updates.entry(app).or_default()) += 1;
        }
    };
    for patch in after {
        match old.get(patch.spec.id.as_str()) {
            None => count(patch, |updates| &mut updates.added),
            Some(old) if old.fingerprint != patch.fingerprint => {
                count(patch, |updates| &mut updates.changed);
            }
            Some(_) => {}
        }
    }
    for patch in before
        .iter()
        .filter(|patch| !new.contains_key(patch.spec.id.as_str()))
    {
        count(patch, |updates| &mut updates.removed);
    }
    updates
}

fn apps(compatibility: &Compatibility) -> Vec<App> {
    match compatibility {
        Compatibility::Universal => vec![App::Any],
        Compatibility::Packages { packages } => packages
            .iter()
            .map(|package| App::Package(package.package.clone()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use reseam_model::{CompatiblePackage, PatchSpec};

    use super::*;

    fn patch(id: &str, compatibility: Compatibility, fingerprint: &str) -> PatchMetadata {
        PatchMetadata {
            spec: PatchSpec {
                bundle: "example-patches".into(),
                id: id.into(),
                name: id.into(),
                hidden: false,
                description: String::new(),
                enabled_by_default: true,
                dependencies: Vec::new(),
                compatibility,
                options: Vec::new(),
                rewrites_output: false,
                translations: Vec::new(),
            },
            incompatibility: None,
            presets: Vec::new(),
            fingerprint: fingerprint.into(),
        }
    }

    fn package(name: &str) -> Compatibility {
        Compatibility::Packages {
            packages: vec![CompatiblePackage {
                package: name.into(),
                versions: Vec::new(),
                version_codes: Vec::new(),
            }],
        }
    }

    #[test]
    fn a_release_updates_the_apps_whose_patches_changed_came_or_went() {
        let before = [
            patch("feed", package("com.example.feed"), "1"),
            patch("ads", package("com.example.feed"), "1"),
            patch("chat", package("com.example.chat"), "1"),
            patch("gone", package("com.example.old"), "1"),
            patch("signature", Compatibility::Universal, "1"),
        ];
        let after = [
            patch("feed", package("com.example.feed"), "2"),
            patch("ads", package("com.example.feed"), "1"),
            patch("chat", package("com.example.chat"), "1"),
            patch("fresh", package("com.example.new"), "1"),
            patch("signature", Compatibility::Universal, "2"),
        ];
        let updates = |changed, added, removed| Updates {
            changed,
            added,
            removed,
        };
        assert_eq!(
            super::updates(&before, &after),
            BTreeMap::from([
                (App::Package("com.example.feed".into()), updates(1, 0, 0)),
                (App::Package("com.example.new".into()), updates(0, 1, 0)),
                (App::Package("com.example.old".into()), updates(0, 0, 1)),
                (App::Any, updates(1, 0, 0)),
            ])
        );
    }
}
