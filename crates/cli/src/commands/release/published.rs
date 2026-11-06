// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bundles of published releases: downloaded and checked against the index that names them.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use reseam_model::{InspectRequest, Trust};
use reseam_sdk::inspect;
use url::Url;

use super::http;
use super::index::unprefixed;

/// What a downloaded bundle must be to stand for the release the index names.
pub(crate) struct Expected<'a> {
    pub(crate) name: &'a str,
    pub(crate) version: &'a str,
    pub(crate) signer: String,
}

pub(crate) fn download(url: &Url, path: &Path, expected: &Expected<'_>) -> Result<()> {
    let directory = path.parent().context("a bundle path has a folder")?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    io::copy(&mut open(url)?, &mut temporary).with_context(|| format!("downloading {url}"))?;
    verify(temporary.path(), expected).with_context(|| format!("checking {url}"))?;
    temporary.persist(path)?;
    Ok(())
}

/// Fails unless `path` is the signed release `expected` describes.
pub(crate) fn verify(path: &Path, expected: &Expected<'_>) -> Result<()> {
    let mut response = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![path.display().to_string()],
        trust: Trust {
            keys: vec![expected.signer.clone()],
        },
    })?;
    let bundle = response.bundles.remove(0);
    if let Some(problem) = bundle.problem {
        bail!("{problem}");
    }
    ensure!(
        bundle.trusted && bundle.public_key == expected.signer,
        "the bundle is signed by {}, not the index signer {}",
        bundle.public_key,
        expected.signer
    );
    ensure!(
        bundle.name == expected.name && unprefixed(&bundle.version) == expected.version,
        "the bundle is {} {}, not the indexed {} {}",
        bundle.name,
        bundle.version,
        expected.name,
        expected.version
    );
    Ok(())
}

/// Reads `url` over HTTP, or from disk for a `file:` URL.
pub(crate) fn open(url: &Url) -> Result<Box<dyn Read>> {
    if url.scheme() == "file" {
        let path: PathBuf = url
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("{url} is not a local path"))?;
        return Ok(Box::new(
            File::open(&path).with_context(|| format!("failed to open {}", path.display()))?,
        ));
    }
    let response = http::checked(http::agent().get(url.as_str()).call())?;
    Ok(Box::new(response.into_body().into_reader()))
}
