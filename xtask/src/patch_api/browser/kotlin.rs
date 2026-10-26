// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Write as _;

use anyhow::{Result, bail};
use boltffi_backend::bridge::jni::JniType;

use super::REVISION_SUFFIX;
use crate::patch_api::jni::Jni;

// Arrays cross CheerpJ by reference, preserving every bit in either direction.
// Keeping the current revision in Java also avoids a LiveConnect call for
// each metadata cache check; native responses update the shared long array.
pub const HEADER: &str = "// Generated from BoltFFI Binding IR. Do not edit.\npackage app.reseam.patch.native\ninternal object Native {\n    private val browserRevision = ThreadLocal<LongArray>()\n    @JvmStatic fun beginBrowserInvocation(value: LongArray) { check(browserRevision.get() == null); browserRevision.set(value) }\n    @JvmStatic fun endBrowserInvocation() { browserRevision.remove() }\n    fun ensureInitialized() {}\n";

/// Declares one native method on the `CheerpJ` `Native` object, with a long-array adapter when it is wide.
pub fn method(
    kotlin: &mut String,
    java_name: &str,
    parameters: &[Jni],
    returns: Jni,
    wide: bool,
) -> Result<()> {
    if java_name.ends_with(REVISION_SUFFIX) {
        writeln!(
            kotlin,
            "    @JvmStatic fun {java_name}(): Long = browserRevision.get()?.get(0) ?: {java_name}_browser()[0]"
        )?;
        writeln!(
            kotlin,
            "    @JvmStatic external fun {java_name}_browser(): LongArray"
        )?;
        return Ok(());
    }
    let native_name = if wide {
        format!("{java_name}_browser")
    } else {
        java_name.to_owned()
    };
    if wide {
        let arguments = parameters
            .iter()
            .enumerate()
            .map(|(index, &slot)| {
                if slot == Jni::LONG {
                    format!("longArrayOf(arg{index})")
                } else {
                    format!("arg{index}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let result = if returns == Jni::Void { "" } else { "return " };
        let unwrap = if returns == Jni::LONG { "[0]" } else { "" };
        writeln!(
            kotlin,
            "    @JvmStatic fun {java_name}({}): {} {{ {result}{native_name}({arguments}){unwrap} }}",
            kotlin_parameters(parameters.iter().copied())?,
            kotlin_type(returns)?
        )?;
    }
    writeln!(
        kotlin,
        "    @JvmStatic external fun {native_name}({}): {}",
        kotlin_parameters(parameters.iter().map(|slot| slot.transported()))?,
        kotlin_type(returns.transported())?
    )?;
    Ok(())
}

fn kotlin_parameters(parameters: impl Iterator<Item = Jni>) -> Result<String> {
    Ok(parameters
        .enumerate()
        .map(|(index, slot)| Ok(format!("arg{index}: {}", kotlin_type(slot)?)))
        .collect::<Result<Vec<_>>>()?
        .join(", "))
}

fn kotlin_type(slot: Jni) -> Result<&'static str> {
    Ok(match slot {
        Jni::Void => "Unit",
        Jni::Scalar(JniType::Boolean) => "Boolean",
        Jni::Scalar(JniType::Byte) => "Byte",
        Jni::Scalar(JniType::Short) => "Short",
        Jni::Scalar(JniType::Int) => "Int",
        Jni::Scalar(JniType::Long) => "Long",
        Jni::Scalar(JniType::Float) => "Float",
        Jni::Scalar(JniType::Double) => "Double",
        Jni::Array(JniType::Byte) => "ByteArray?",
        Jni::Array(JniType::Short) => "ShortArray",
        Jni::Array(JniType::Int) => "IntArray",
        Jni::Array(JniType::Long) => "LongArray",
        Jni::ByteBuffer => "java.nio.ByteBuffer",
        other => bail!("unsupported browser Java type: {}", other.signature()),
    })
}
