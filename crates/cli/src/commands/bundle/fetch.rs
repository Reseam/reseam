// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use reseam_model::{InspectRequest, Trust};
use reseam_sdk::inspect;
use url::Url;

use super::super::release::http;
use super::super::release::index::{Index, unprefixed};
use crate::app::BundleFetchCommand;

/// What a downloaded bundle must be to stand for the release the index names.
struct Expected<'a> {
    name: &'a str,
    version: &'a str,
    signer: String,
}

pub fn run_bundle_fetch(command: &BundleFetchCommand) -> Result<()> {
    let index_url = Url::parse(&command.index).context("--index must be a URL")?;
    let mut json = String::new();
    open(&index_url)?.read_to_string(&mut json)?;
    let index = Index::parse(&json)?;
    let signer = index
        .public_key()
        .with_context(|| format!("{index_url} names no bundle public key"))?;
    if let Some(pinned) = &command.signer {
        ensure!(
            pinned.to_ascii_lowercase() == signer,
            "{index_url} is signed by {signer}, not the pinned {pinned}"
        );
    }
    let release = index.release(&command.version)?;
    let expected = Expected {
        name: index.name(),
        version: unprefixed(&release.version),
        signer,
    };
    let file_name = format!("{}-{}.reseam", expected.name, expected.version);
    ensure!(
        Path::new(&file_name).file_name() == Some(file_name.as_ref()) && !file_name.contains('\\'),
        "{index_url} names a bundle that is not a file name: {file_name}"
    );
    fs::create_dir_all(&command.to)
        .with_context(|| format!("failed to create {}", command.to.display()))?;
    let path = command.to.join(file_name);
    if !(path.is_file() && verify(&path, &expected).is_ok()) {
        download(&index_url.join(&release.download_url)?, &path, &expected)?;
    }
    println!("{}", path.display());
    Ok(())
}

fn download(url: &Url, path: &Path, expected: &Expected<'_>) -> Result<()> {
    let directory = path.parent().context("a bundle path has a folder")?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    io::copy(&mut open(url)?, &mut temporary).with_context(|| format!("downloading {url}"))?;
    verify(temporary.path(), expected).with_context(|| format!("checking {url}"))?;
    temporary.persist(path)?;
    Ok(())
}

/// Fails unless `path` is the signed release `expected` describes.
fn verify(path: &Path, expected: &Expected<'_>) -> Result<()> {
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
fn open(url: &Url) -> Result<Box<dyn Read>> {
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
