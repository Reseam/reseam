// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use reseam_sdk::authoring::{TEMPLATE_URL, init_bundle};

use super::super::release::http;
use crate::app::BundleInitCommand;

pub fn run_bundle_init(command: &BundleInitCommand) -> Result<()> {
    let archive = http::checked(http::agent().get(TEMPLATE_URL).call())?
        .into_body()
        .read_to_vec()
        .with_context(|| format!("downloading {TEMPLATE_URL}"))?;
    init_bundle(
        Cursor::new(archive),
        &command.name,
        Path::new(&command.name),
    )?;
    println!("Created bundle {} in {}/", command.name, command.name);
    Ok(())
}
