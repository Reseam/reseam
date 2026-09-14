// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generates the hosted patch bridge from the patcher's Binding IR: Kotlin,
//! the C JNI glue and header, and the registration table the engine uses to
//! bind that glue to a bundle's `Native` class.

use std::collections::BTreeSet;
use std::fs;

use anyhow::{bail, ensure, Context, Result};
use boltffi_backend::bridge::jni::{JniBridgeContract, NativeParameterKind, NativeReturn};
use boltffi_backend::core::bridge::BridgeStack;
use boltffi_backend::target::kotlin::{KotlinDesktopLoader, KotlinHost};
use boltffi_bindgen::{generate::Generation, metadata::BindingMetadataBuild};
use boltffi_binding::{BindingMetadataSurface, Bindings, DeclarationRef, Native, Surface};

use crate::paths;

pub fn regen() -> Result<()> {
    let bindings = BindingMetadataBuild::new(paths::patcher_crate().join("Cargo.toml"))
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
    let output = paths::patch_api().join("generated");
    Generation::write_output(target.render(&bindings)?, &output)?;
    fs::write(output.join("jni/registration.c"), registration(&contract)?)?;
    println!("Generated the patch bridge from Binding IR.");
    Ok(())
}

/// Declarations reachable from exported calls. Dependency crates carry
/// application models the hosted patch API never sees.
fn reachable(bindings: &Bindings<Native>) -> BTreeSet<boltffi_binding::DeclarationId> {
    let mut required = bindings
        .decls()
        .iter()
        .filter(|decl| decl.exported_callables().next().is_some())
        .map(|decl| decl.id())
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
            .map(|decl| decl.id())
            .collect::<Vec<_>>();
        let before = required.len();
        required.extend(referenced);
        if required.len() == before {
            return required;
        }
    }
}

/// A translation unit that includes the untouched glue and registers its
/// entry points on the class the engine supplies. Only synchronous calls run
/// inside a hosted patch context.
fn registration(contract: &JniBridgeContract) -> Result<String> {
    ensure!(
        contract.callbacks().is_empty()
            && contract.closures().is_empty()
            && contract.streams().is_empty()
            && contract.callback_completions().is_empty()
            && contract.success_out_writers().is_empty()
            && contract.callback_handle_lifecycle().is_none(),
        "hosted patch bridge must contain only synchronous methods"
    );
    let entries = contract
        .methods()
        .iter()
        .map(|method| {
            let parameters = method
                .parameters()
                .iter()
                .map(|parameter| {
                    Ok(match parameter.kind() {
                        NativeParameterKind::Scalar(value) => value.ty().signature(),
                        NativeParameterKind::Bytes(_) => "Ljava/nio/ByteBuffer;I",
                        NativeParameterKind::Record(_) => "Ljava/nio/ByteBuffer;",
                        NativeParameterKind::DirectVector(value) => {
                            value.jni_type().array_signature()
                        }
                        other => bail!("unsupported hosted JNI parameter: {other:?}"),
                    })
                })
                .collect::<Result<String>>()?;
            let returns = return_signature(method.returns())?;
            Ok(format!(
                "    {{\"{}\", \"({parameters}){returns}\", (void *){}}},",
                method.c_function().name(),
                method.symbol()
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    Ok(format!(
        r#"/* Generated from BoltFFI Binding IR. Do not edit. */
#include "jni_glue.c"

jint reseam_register_patch_natives(JNIEnv *env, jclass native_class) {{
    const JNINativeMethod methods[] = {{
{entries}
    }};
    return (*env)->RegisterNatives(env, native_class, methods,
        (jint)(sizeof(methods) / sizeof(methods[0])));
}}
"#
    ))
}

fn return_signature(value: &NativeReturn) -> Result<&'static str> {
    Ok(match value {
        NativeReturn::Void | NativeReturn::Status | NativeReturn::EncodedError => "V",
        NativeReturn::Value(value) => value.jni_type().signature(),
        NativeReturn::Bytes | NativeReturn::Record(_) | NativeReturn::StatusWriteback(_) => "[B",
        NativeReturn::StatusValue(value) => return_signature(value.value())?,
        NativeReturn::EncodedErrorValue(value) => return_signature(value.success().value())?,
        other => bail!("unsupported hosted JNI return: {other:?}"),
    })
}
