// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generates the hosted patch bridge and its registration table from Binding IR.

use std::fs;

use anyhow::{bail, ensure, Context, Result};
use boltffi_backend::bridge::jni::{JniBridgeContract, NativeParameterKind, NativeReturn};
use boltffi_backend::core::bridge::BridgeStack;
use boltffi_backend::target::kotlin::{KotlinDesktopLoader, KotlinHost};
use boltffi_bindgen::{generate::Generation, metadata::BindingMetadataBuild};
use boltffi_binding::{BindingMetadataSurface, DeclarationRef, Native, Surface};

use crate::paths;

/// Builds one metadata contract and emits Kotlin, C, and native registration from it.
///
/// Patch APIs run synchronously inside a host-owned context. Callback/stream
/// exports are rejected: their JVM lifecycle needs a separately designed host
/// integration. A new JNI parameter or return kind fails regeneration explicitly.
pub fn regen() -> Result<()> {
    let bindings = BindingMetadataBuild::new(paths::patcher_crate().join("Cargo.toml"))
        .surface(BindingMetadataSurface::Native)
        .cargo_environment([("RESEAM_SKIP_JNI_GLUE", "1")])
        .read()?
        .into_iter()
        .find(|envelope| envelope.surface() == BindingMetadataSurface::Native)
        .and_then(|envelope| Native::from_serialized(envelope.into_bindings()))
        .context("patcher build emitted no native Binding IR")?;
    // Emit only types reachable from exported calls. Dependency crates may
    // contain application models that are unrelated to the hosted patch API.
    let mut required = bindings
        .decls()
        .iter()
        .filter(|decl| decl.exported_callables().next().is_some())
        .map(|decl| decl.id())
        .collect::<std::collections::BTreeSet<_>>();
    loop {
        let dependencies = bindings
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
        required.extend(dependencies);
        if required.len() == before {
            break;
        }
    }
    let bindings = bindings.dependency_closed(&required)?;
    let target = KotlinHost::new("app.reseam.patch", "ReseamPatcher")?
        .android_library("reseam_patcher")?
        .desktop_loader(KotlinDesktopLoader::None)
        .c_header("jni/reseam_patcher.h")
        .into_target()?;
    let bridge = target.stack().build(&bindings)?;
    let (contract, _) = bridge.into_parts();
    let registration = registration_source(&contract)?;
    let output = paths::patch_api().join("generated");
    Generation::write_output(target.render(&bindings)?, &output)?;
    fs::write(output.join("jni/registration.c"), registration)?;
    publish_bridge()?;
    println!("Generated patch Kotlin, JNI, header, and registration from one Binding IR.");
    Ok(())
}

/// Emits a C translation unit that includes untouched BoltFFI glue and registers
/// its methods on the exact class supplied by each bundle's classloader.
fn registration_source(contract: &JniBridgeContract) -> Result<String> {
    ensure!(
        contract.callbacks().is_empty()
            && contract.closures().is_empty()
            && contract.streams().is_empty()
            && contract.callback_completions().is_empty()
            && contract.success_out_writers().is_empty()
            && contract.callback_handle_lifecycle().is_none(),
        "hosted patch bridge must contain only synchronous methods"
    );
    ensure!(
        !contract.methods().is_empty(),
        "patch bridge has no native methods"
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

/// Returns a JVM descriptor from the resolved JNI return contract.
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

// BoltFFI's "none" loader setting disables desktop loading only. The embedded
// patch bridge must also omit Android loading because the engine registers the
// natives on the bundle's own class. Keep this small, exact policy adaptation
// until upstream exposes a host-owned loader for both platforms.
const GENERATED_LOADER: &str = r#"    init {
        val androidLibrary = "reseam_patcher"
        val desktopPreferredLibrary = "boltffi_jni"
        val desktopFallbackLibrary = "boltffi"
        val vmName = System.getProperty("java.vm.name").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
            vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary(androidLibrary)
        }
    }"#;

/// Publishes module-private transport functions and public value types.
///
/// Rejects an unexpected loader template rather than silently leaving a second
/// library load in the embedded bridge. Wire codecs and native calls are untouched.
fn publish_bridge() -> Result<()> {
    let root = paths::patch_api();
    let source = fs::read_to_string(root.join("generated/app/reseam/patch/ReseamPatcher.kt"))?;
    ensure!(
        source.matches(GENERATED_LOADER).count() == 1,
        "BoltFFI host loader changed; review the embedded patch loader policy"
    );
    let source = source.replace(GENERATED_LOADER, "");
    let source = source
        .lines()
        .map(|line| {
            if line.starts_with("fun ") {
                format!("internal {line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(root.join("src/main/kotlin/app/reseam/patch/ReseamPatcher.kt"),
        format!("// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n{source}\n"))?;
    Ok(())
}
