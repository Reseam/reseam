// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::io::{Read, Write};

use anyhow::{Context, Result, ensure};
use ed25519_dalek::{SecretKey, SigningKey};
use reseam_patcher::Compatibility;
use reseam_sdk::{InspectRequest, PatchMetadata, inspect};
use ring::rand::{SecureRandom, SystemRandom};
use tracing::info;

use super::create_parent;
use crate::app::{BundleKeygenCommand, BundleListCommand, BundlePackCommand};

pub fn run_bundle_list(command: &BundleListCommand) -> Result<()> {
    let response = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![command.bundle.display().to_string()],
        trust: (&command.trust.store()?).into(),
    })?;
    if command.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    let bundle = &response.bundles[0];
    if let Some(problem) = &bundle.problem {
        anyhow::bail!("{problem}");
    }
    println!("bundle: {}", bundle.name);
    if !bundle.author.is_empty() {
        println!("author: {}", bundle.author);
    }
    if !bundle.description.is_empty() {
        println!("description: {}", bundle.description);
    }
    println!(
        "signer: {} ({})",
        bundle.public_key,
        if bundle.trusted {
            "trusted"
        } else {
            "untrusted"
        }
    );
    if command.verbose {
        println!("engine: {}", bundle.engine);
    }
    println!("files: {}", bundle.files.len());
    if command.verbose {
        for file in &bundle.files {
            println!("  {file}");
        }
    }
    println!();
    let hidden_references: HashSet<_> = response
        .patches
        .iter()
        .filter(|patch| patch.spec.hidden)
        .map(|patch| patch.spec.reference())
        .collect();
    for (index, patch) in response
        .patches
        .iter()
        .filter(|patch| !patch.spec.hidden)
        .enumerate()
    {
        print_patch(index, patch, &hidden_references);
    }
    let hidden = response
        .patches
        .iter()
        .filter(|patch| patch.spec.hidden)
        .count();
    if hidden > 0 {
        println!();
        println!("{hidden} internal patch(es) run as dependencies and are not listed");
    }
    Ok(())
}

fn print_patch(index: usize, patch: &PatchMetadata, hidden: &HashSet<String>) {
    let spec = &patch.spec;
    println!(
        "  {:>3}. [{}] {} - {}",
        index + 1,
        if spec.enabled_by_default { "on" } else { "off" },
        spec.name,
        spec.description
    );
    println!("       id: {}", spec.id);
    match &spec.compatibility {
        Compatibility::Universal => println!("       packages: any app"),
        Compatibility::Packages { packages } => {
            let packages: Vec<String> = packages
                .iter()
                .map(|entry| {
                    if entry.versions.is_empty() {
                        entry.package.clone()
                    } else {
                        format!("{} ({})", entry.package, entry.versions.join(", "))
                    }
                })
                .collect();
            println!("       packages: {}", packages.join(", "));
        }
    }
    let dependencies: Vec<&str> = spec
        .dependencies
        .iter()
        .filter(|reference| !hidden.contains(*reference))
        .map(String::as_str)
        .collect();
    if !dependencies.is_empty() {
        println!("       depends: {}", dependencies.join(", "));
    }
    if !spec.options.is_empty() {
        println!("       options:");
        for option in &spec.options {
            println!(
                "         - {} ({:?}, {})",
                option.key,
                option.option_type,
                if option.required {
                    "required"
                } else {
                    "optional"
                }
            );
        }
    }
}

pub fn run_bundle_keygen(command: &BundleKeygenCommand) -> Result<()> {
    ensure!(
        !command.out.exists(),
        "refusing to overwrite existing key at {}",
        command.out.display()
    );
    create_parent(&command.out)?;
    let mut seed = [0u8; 32];
    SystemRandom::new()
        .fill(&mut seed)
        .map_err(|error| anyhow::anyhow!("generate signing seed: {error}"))?;
    let parent = command
        .out
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(&seed)?;
    staged
        .persist_noclobber(&command.out)
        .map_err(|error| error.error)
        .with_context(|| format!("publish key {}", command.out.display()))?;

    println!("Ed25519 keypair generated");
    println!("  private seed: {}", command.out.display());
    println!(
        "  public key (hex): {}",
        hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes())
    );
    Ok(())
}

pub fn run_bundle_pack(command: &BundlePackCommand) -> Result<()> {
    let mut file = std::fs::File::open(&command.key)
        .with_context(|| format!("read key {}", command.key.display()))?;
    let mut seed: SecretKey = [0; 32];
    file.read_exact(&mut seed)
        .context("signing key must be exactly 32 bytes")?;
    ensure!(
        file.read(&mut [0])? == 0,
        "signing key must be exactly 32 bytes"
    );
    create_parent(&command.out)?;
    reseam_patcher::bundle::pack(&command.dir, &SigningKey::from_bytes(&seed), &command.out)?;
    info!(out = %command.out.display(), "bundle packed and signed");
    Ok(())
}
