// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, ensure};
use reseam_model::{PublishedRelease, Publisher, ReleaseIndex};

pub(crate) fn parse(json: &str) -> Result<ReleaseIndex> {
    serde_json::from_str(json).context("the published index is not a release index")
}

pub(crate) fn to_json(index: &ReleaseIndex) -> Result<String> {
    Ok(serde_json::to_string_pretty(index)? + "\n")
}

/// The signer the publisher declares, in lowercase hex.
pub(crate) fn public_key(index: &ReleaseIndex) -> Option<String> {
    index
        .bundle
        .public_key
        .as_deref()
        .map(str::to_ascii_lowercase)
}

/// The release named `version`; a leading `v` on either name is ignored.
pub(crate) fn release<'a>(index: &'a ReleaseIndex, version: &str) -> Result<&'a PublishedRelease> {
    let wanted = unprefixed(version);
    let mut matching = index
        .releases
        .iter()
        .filter(|release| unprefixed(&release.version) == wanted);
    let release = matching
        .next()
        .with_context(|| format!("the index has no release {wanted}"))?;
    ensure!(
        matching.next().is_none(),
        "the index lists release {wanted} more than once"
    );
    Ok(release)
}

/// `previous` with `release` added as the newest, published by `publisher`.
pub(crate) fn add(
    previous: Option<ReleaseIndex>,
    publisher: Publisher,
    release: PublishedRelease,
) -> Result<ReleaseIndex> {
    let mut releases = match previous {
        Some(previous) => {
            ensure!(
                previous.bundle.public_key == publisher.public_key,
                "refusing to change the public key of the published index (published {:?}, new {:?})",
                previous.bundle.public_key,
                publisher.public_key
            );
            previous.releases
        }
        None => Vec::new(),
    };
    releases.retain(|existing| existing.version != release.version);
    releases.insert(0, release);
    // Clients read patches only from the newest stable release and the newest prerelease; older
    // lists would grow the index by the whole catalog on every release.
    let (mut stable_listed, mut prerelease_listed) = (false, false);
    for release in &mut releases {
        let listed = if release.prerelease {
            &mut prerelease_listed
        } else {
            &mut stable_listed
        };
        if std::mem::replace(listed, true) {
            release.patches = None;
        }
    }
    Ok(ReleaseIndex {
        bundle: publisher,
        releases,
    })
}

/// A version without the `v` that tags often carry.
pub(crate) fn unprefixed(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publisher(key: &str) -> Publisher {
        Publisher {
            name: "example-patches".into(),
            author: "Example".into(),
            description: String::new(),
            homepage: None,
            public_key: Some(key.into()),
        }
    }

    fn entry(version: &str, prerelease: bool) -> PublishedRelease {
        PublishedRelease {
            version: version.into(),
            created_at: "2026-10-01T00:00:00Z".into(),
            description: version.into(),
            download_url: format!("https://example.com/{version}"),
            prerelease,
            patches: Some(Vec::new()),
        }
    }

    #[test]
    fn only_the_newest_stable_release_and_prerelease_keep_their_patches() {
        let mut index = None;
        for (version, prerelease) in [
            ("1.0.0", false),
            ("1.1.0-beta", true),
            ("1.1.0", false),
            ("1.2.0-beta", true),
            ("1.1.0", false),
        ] {
            index = Some(add(index, publisher("00"), entry(version, prerelease)).unwrap());
        }

        let index = index.unwrap();
        let releases: Vec<_> = index
            .releases
            .iter()
            .map(|release| (release.version.as_str(), release.patches.is_some()))
            .collect();
        assert_eq!(
            releases,
            [
                ("1.1.0", true),
                ("1.2.0-beta", true),
                ("1.1.0-beta", false),
                ("1.0.0", false),
            ]
        );
    }

    #[test]
    fn a_new_key_cannot_take_over_a_published_index() {
        let index = add(None, publisher("00"), entry("1.0.0", false)).unwrap();
        assert!(add(Some(index), publisher("11"), entry("1.1.0", false)).is_err());
    }
}
