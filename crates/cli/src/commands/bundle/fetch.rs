// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use url::Url;

use super::super::release::index::{self, unprefixed};
use super::super::release::published::{Expected, download, open, verify};
use crate::app::BundleFetchCommand;

pub fn run_bundle_fetch(command: &BundleFetchCommand) -> Result<()> {
    let index_url = Url::parse(&command.index).context("--index must be a URL")?;
    let mut json = String::new();
    open(&index_url)?.read_to_string(&mut json)?;
    let index = index::parse(&json)?;
    let signer = index::public_key(&index)
        .with_context(|| format!("{index_url} names no bundle public key"))?;
    if let Some(pinned) = &command.signer {
        ensure!(
            pinned.to_ascii_lowercase() == signer,
            "{index_url} is signed by {signer}, not the pinned {pinned}"
        );
    }
    let release = index::release(&index, &command.version)?;
    let expected = Expected {
        name: &index.bundle.name,
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
