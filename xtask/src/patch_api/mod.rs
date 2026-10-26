// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod browser;
mod jni;
mod kotlin;
mod opcodes;
mod registration;

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use boltffi_backend::core::bridge::BridgeStack;
use boltffi_backend::target::kotlin::{KotlinDesktopLoader, KotlinHost};
use boltffi_bindgen::{generate::Generation, metadata::BindingMetadataBuild};
use boltffi_binding::{BindingMetadataSurface, Bindings, Decl, DeclarationRef, Native, Surface};

use crate::paths;

pub fn regen() -> Result<()> {
    let bindings = BindingMetadataBuild::new(
        paths::workspace_root()
            .join("crates/patcher")
            .join("Cargo.toml"),
    )
    .surface(BindingMetadataSurface::Native)
    .read()?
    .into_iter()
    .find(|envelope| envelope.surface() == BindingMetadataSurface::Native)
    .and_then(|envelope| Native::from_serialized(envelope.into_bindings()))
    .context("patcher build emitted no native Binding IR")?;
    let bindings = bindings.dependency_closed(&reachable(&bindings))?;
    // Android hosts share the SDK library; every other host registers the
    // bridge itself, so the generated loader must not load anything there.
    let target = KotlinHost::new("app.reseam.patch.native", "ReseamPatcher")?
        .android_library("reseam-sdk-native")?
        .desktop_loader(KotlinDesktopLoader::None)
        .c_header("jni/reseam_patcher.h")
        .into_target()?;
    let (contract, _) = target.stack().build(&bindings)?.into_parts();
    let output = paths::workspace_root().join("patch-api").join("generated");
    // Regeneration replaces the whole tree, so files a previous generator wrote do not linger.
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    Generation::write_output(target.render(&bindings)?, &output)?;
    fs::write(
        output.join("jni/registration.c"),
        registration::source(&contract)?,
    )?;
    browser::write(&contract, &output)?;
    kotlin::split_surface(&output)?;
    opcodes::write(&output.join("app/reseam/patch/dex"))?;
    writeln!(
        std::io::stdout(),
        "Generated the patch bridge from Binding IR."
    )?;
    Ok(())
}

fn reachable(bindings: &Bindings<Native>) -> BTreeSet<boltffi_binding::DeclarationId> {
    let mut required = bindings
        .decls()
        .iter()
        .filter(|decl| decl.exported_callables().next().is_some())
        .map(Decl::id)
        .collect::<BTreeSet<_>>();
    loop {
        let referenced = bindings
            .decls()
            .iter()
            .filter(|candidate| {
                bindings
                    .decls()
                    .iter()
                    .filter(|decl| required.contains(&decl.id()))
                    .any(|decl| DeclarationRef::from(decl).references_declaration(candidate.id()))
            })
            .map(Decl::id)
            .collect::<Vec<_>>();
        let before = required.len();
        required.extend(referenced);
        if required.len() == before {
            return required;
        }
    }
}
