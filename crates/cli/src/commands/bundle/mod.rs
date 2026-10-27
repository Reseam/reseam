// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod fetch;

use std::collections::HashSet;

use anyhow::Result;
use reseam_model::{Compatibility, InspectRequest, PatchMetadata, PatchRef};
use reseam_sdk::authoring::{BundleKey, manifest_info, pack};
use reseam_sdk::inspect;
use tracing::info;

pub use self::fetch::run_bundle_fetch;
use super::create_parent;
use crate::app::{
    BundleKeygenCommand, BundleListCommand, BundleManifestCommand, BundlePackCommand,
    BundlePubkeyCommand,
};

pub fn run_bundle_list(command: &BundleListCommand) -> Result<()> {
    let response = inspect(&InspectRequest {
        apk_path: None,
        split_paths: Vec::new(),
        bundle_paths: vec![command.bundle.display().to_string()],
        trust: command.trust.trust(),
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
    if !bundle.version.is_empty() {
        println!("version: {}", bundle.version);
    }
    if !bundle.author.is_empty() {
        println!("author: {}", bundle.author);
    }
    if !bundle.description.is_empty() {
        println!("description: {}", bundle.description);
    }
    if !bundle.license.is_empty() {
        println!("license: {}", bundle.license);
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
        .map(|patch| &patch.spec.reference)
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

fn print_patch(index: usize, patch: &PatchMetadata, hidden: &HashSet<&PatchRef>) {
    let spec = &patch.spec;
    println!(
        "  {:>3}. [{}] {} - {}",
        index + 1,
        if spec.enabled_by_default { "on" } else { "off" },
        spec.name,
        spec.description
    );
    println!("       id: {}", spec.reference.id);
    match &spec.compatibility {
        Compatibility::Universal => println!("       packages: any app"),
        Compatibility::Packages { packages } => {
            let packages: Vec<String> = packages
                .iter()
                .map(|entry| {
                    let versions: Vec<String> = entry
                        .versions
                        .iter()
                        .cloned()
                        .chain(
                            entry
                                .version_codes
                                .iter()
                                .map(|code| format!("code {code}")),
                        )
                        .collect();
                    if versions.is_empty() {
                        entry.package.clone()
                    } else {
                        format!("{} ({})", entry.package, versions.join(", "))
                    }
                })
                .collect();
            println!("       packages: {}", packages.join(", "));
        }
    }
    let dependencies: Vec<String> = spec
        .dependencies
        .iter()
        .filter(|reference| !hidden.contains(reference))
        .map(ToString::to_string)
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
    create_parent(&command.out)?;
    let key = BundleKey::generate(&command.out)?;
    println!("Ed25519 keypair generated");
    println!("  private seed: {}", command.out.display());
    println!("  public key (hex): {}", hex::encode(key.public_key()));
    Ok(())
}

pub fn run_bundle_pubkey(command: &BundlePubkeyCommand) -> Result<()> {
    println!(
        "{}",
        hex::encode(BundleKey::read(&command.key)?.public_key())
    );
    Ok(())
}

pub fn run_bundle_pack(command: &BundlePackCommand) -> Result<()> {
    let key = BundleKey::read(&command.key)?;
    create_parent(&command.out)?;
    pack(&command.dir, command.version.as_deref(), &key, &command.out)?;
    info!(out = %command.out.display(), "bundle packed and signed");
    Ok(())
}

pub fn run_bundle_manifest(command: &BundleManifestCommand) -> Result<()> {
    println!("{}", serde_json::to_string(&manifest_info(&command.path)?)?);
    Ok(())
}
