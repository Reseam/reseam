// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Context, Result};
use reseam_library::{built_in_trust_store, inspect_with_trust};
use tracing::info;

use crate::app::{BundleKeygenCommand, BundleListCommand, BundlePackCommand};

pub fn run_bundle_list(command: &BundleListCommand) -> Result<()> {
    let response = inspect_with_trust(
        std::slice::from_ref(&command.bundle),
        None,
        &[],
        &built_in_trust_store(),
    )?;
    let metadata = response
        .bundles
        .into_iter()
        .next()
        .context("bundle metadata missing from inspect response")?;
    println!("bundle: {}", metadata.name);
    if !metadata.author.is_empty() {
        println!("author: {}", metadata.author);
    }
    if !metadata.description.is_empty() {
        println!("description: {}", metadata.description);
    }
    println!();
    for (index, patch) in response.patches.iter().enumerate() {
        let enabled = if patch.enabled_by_default {
            "on"
        } else {
            "off"
        };
        println!(
            "  {:>3}. [{}] {} - {}",
            index + 1,
            enabled,
            patch.name,
            patch.description
        );

        if !patch.compatible_with.is_empty() {
            let formatted: Vec<String> = patch
                .compatible_with
                .iter()
                .map(|compatibility| {
                    if compatibility.versions.is_empty() {
                        compatibility.package_name.clone()
                    } else {
                        format!(
                            "{} ({})",
                            compatibility.package_name,
                            compatibility.versions.join(", ")
                        )
                    }
                })
                .collect();
            println!("       packages: {}", formatted.join(", "));
        }

        if !patch.dependencies.is_empty() {
            println!("       depends: {}", patch.dependencies.join(", "));
        }

        if !patch.options.is_empty() {
            println!("       options:");
            for option in &patch.options {
                let required = if option.required {
                    "required"
                } else {
                    "optional"
                };
                println!(
                    "         - {} ({:?}, {})",
                    option.key, option.option_type, required
                );
            }
        }
    }

    if !metadata.extension_dex.is_empty() {
        println!();
        println!("extension DEX:");
        for dex_path in &metadata.extension_dex {
            println!("  - {}", dex_path);
        }
    }

    Ok(())
}

pub fn run_bundle_keygen(command: &BundleKeygenCommand) -> Result<()> {
    use rand::RngCore;
    use std::os::unix::fs::OpenOptionsExt;

    if command.out.exists() {
        bail!(
            "refusing to overwrite existing key at {}",
            command.out.display()
        );
    }
    if let Some(parent) = command
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let public_key = signing_key.verifying_key().to_bytes();

    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&command.out)
        .with_context(|| format!("failed to create {}", command.out.display()))?;
    use std::io::Write as _;
    file.write_all(&seed).context("failed to write seed")?;

    println!("Ed25519 keypair generated");
    println!("  private seed: {}", command.out.display());
    println!("  public key (hex): {}", hex::encode(public_key));
    println!("  trust this signer in your client before loading its bundles");
    Ok(())
}

pub fn run_bundle_pack(command: &BundlePackCommand) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;

    let manifest_path = command.dir.join("manifest.toml");
    let manifest_source = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    #[derive(serde::Deserialize)]
    struct PartialManifest {
        bundle: PartialBundle,
    }

    #[derive(serde::Deserialize)]
    struct PartialBundle {
        name: String,
        #[serde(default)]
        author: String,
        #[serde(default)]
        description: String,
        format_version: u32,
    }

    let partial: PartialManifest =
        toml::from_str(&manifest_source).context("failed to parse manifest.toml")?;
    if partial.bundle.format_version != reseam_patcher::bundle::BUNDLE_FORMAT_VERSION {
        bail!(
            "unsupported format_version {}; CLI supports {}",
            partial.bundle.format_version,
            reseam_patcher::bundle::BUNDLE_FORMAT_VERSION
        );
    }

    let mut payload = Vec::new();
    for entry in std::fs::read_dir(&command.dir)
        .with_context(|| format!("read {}", command.dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("non-utf8 filename")?
            .to_string();
        if name == "manifest.toml" {
            continue;
        }
        let lowercase = name.to_ascii_lowercase();
        if !(lowercase.ends_with(".jar")
            || lowercase.ends_with(".dex")
            || lowercase.ends_with(".rve"))
        {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        payload.push((name, bytes));
    }
    payload.sort_by(|left, right| left.0.cmp(&right.0));

    if payload.is_empty() {
        bail!("no .jar/.dex/.rve files found in {}", command.dir.display());
    }

    let mut manifest = String::new();
    manifest.push_str("[bundle]\n");
    manifest.push_str(&format!("name = {}\n", toml_string(&partial.bundle.name)));
    if !partial.bundle.author.is_empty() {
        manifest.push_str(&format!(
            "author = {}\n",
            toml_string(&partial.bundle.author)
        ));
    }
    if !partial.bundle.description.is_empty() {
        manifest.push_str(&format!(
            "description = {}\n",
            toml_string(&partial.bundle.description)
        ));
    }
    manifest.push_str(&format!(
        "format_version = {}\n\n",
        partial.bundle.format_version
    ));
    manifest.push_str("[files]\n");
    for (name, bytes) in &payload {
        manifest.push_str(&format!(
            "{} = \"{}\"\n",
            toml_string(name),
            hex::encode(Sha256::digest(bytes))
        ));
    }
    let manifest_bytes = manifest.into_bytes();

    let seed = std::fs::read(&command.key)
        .with_context(|| format!("read key {}", command.key.display()))?;
    if seed.len() != 32 {
        bail!("signing key must be exactly 32 bytes, got {}", seed.len());
    }
    let seed: [u8; 32] = seed
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("signing key must be exactly 32 bytes"))?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let public_key = signing_key.verifying_key().to_bytes();
    let signature = ed25519_dalek::Signer::sign(&signing_key, &manifest_bytes).to_bytes();

    if let Some(parent) = command
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(&command.out)
        .with_context(|| format!("create {}", command.out.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", stored)?;
    zip.write_all(reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes())?;
    zip.start_file("manifest.toml", deflated)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file("manifest.pubkey", stored)?;
    zip.write_all(&public_key)?;
    zip.start_file("manifest.sig", stored)?;
    zip.write_all(&signature)?;
    for (name, bytes) in &payload {
        zip.start_file(name, deflated)?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;

    info!(
        bundle = %partial.bundle.name,
        out = %command.out.display(),
        file_count = payload.len(),
        "bundle packed and signed"
    );
    Ok(())
}

fn toml_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if (ch as u32) < 0x20 => output.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output.push('"');
    output
}
