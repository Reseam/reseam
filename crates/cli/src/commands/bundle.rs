// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{ensure, Context, Result};
use ed25519_dalek::SigningKey;
use rand::RngCore;
use reseam_sdk::{inspect, InspectRequest};
use tracing::info;

use super::create_parent;
use crate::app::{BundleKeygenCommand, BundleListCommand, BundlePackCommand};

pub fn run_bundle_list(command: &BundleListCommand) -> Result<()> {
    let response = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![command.bundle.clone()],
        trust: command.trust.store()?,
    })?;
    let bundle = &response.bundles[0];
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
    println!("engine: {}", bundle.engine);
    println!("files: {}", bundle.files.join(", "));
    if !bundle.trusted {
        println!("patches are not listed for untrusted signers");
        return Ok(());
    }
    println!();
    for (index, patch) in response.patches.iter().enumerate() {
        let spec = &patch.spec;
        println!(
            "  {:>3}. [{}] {} - {}",
            index + 1,
            if spec.enabled_by_default { "on" } else { "off" },
            spec.id,
            spec.description
        );
        if !spec.compatibility.is_empty() {
            let packages: Vec<String> = spec
                .compatibility
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
        if !spec.dependencies.is_empty() {
            println!("       depends: {}", spec.dependencies.join(", "));
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
    Ok(())
}

pub fn run_bundle_keygen(command: &BundleKeygenCommand) -> Result<()> {
    ensure!(
        !command.out.exists(),
        "refusing to overwrite existing key at {}",
        command.out.display()
    );
    create_parent(&command.out)?;
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&command.out)
        .with_context(|| format!("failed to create {}", command.out.display()))?
        .write_all(&seed)?;

    println!("Ed25519 keypair generated");
    println!("  private seed: {}", command.out.display());
    println!(
        "  public key (hex): {}",
        hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes())
    );
    Ok(())
}

pub fn run_bundle_pack(command: &BundlePackCommand) -> Result<()> {
    let seed = std::fs::read(&command.key)
        .with_context(|| format!("failed to read key {}", command.key.display()))?;
    let seed: [u8; 32] = seed
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("signing key must be exactly 32 bytes"))?;
    create_parent(&command.out)?;
    reseam_patcher::bundle::pack(&command.dir, &SigningKey::from_bytes(&seed), &command.out)?;
    info!(out = %command.out.display(), "bundle packed and signed");
    Ok(())
}
