// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::Result;
use boltffi_backend::bridge::jni::{JniBridgeContract, NativeMethod, NativeParameterKind};
use serde::Serialize;

use super::jni::{self, Jni};
use super::registration;

mod kotlin;

const REVISION_SUFFIX: &str = "_changes_mutation_revision";

/// One entry of `methods.json`, which the browser workers dispatch native calls by.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeMethod {
    buffer_lengths: BTreeMap<usize, usize>,
    export: String,
    name: String,
    parameters: Vec<&'static str>,
    returns: &'static str,
}

/// Writes the WASM exports, the `CheerpJ` `Native` object and the method manifest of the browser bridge.
pub fn write(contract: &JniBridgeContract, output: &Path) -> Result<()> {
    let mut source = String::from(
        "/* Generated from BoltFFI Binding IR. Do not edit. */\n#include \"transport.c\"\n#include \"registration.c\"\n",
    );
    let mut kotlin = String::from(kotlin::HEADER);
    let mut methods = Vec::new();
    for (index, method) in contract.methods().iter().enumerate() {
        let parameters = jni::parameters(method)?;
        let returns = jni::returns(method.returns())?;
        let wide = returns == Jni::LONG || parameters.contains(&Jni::LONG);
        let java_name = method.c_function().name();
        let original = method.symbol().to_string();
        kotlin::method(&mut kotlin, java_name, &parameters, returns, wide)?;
        let export = format!("reseam_bridge_{index}");
        let call = if jni::is_checked(method) {
            registration::checked_name(index)
        } else {
            original.clone()
        };
        source.push_str(&c_method(&export, &call, &parameters, returns));
        if java_name.ends_with(REVISION_SUFFIX) {
            // Read-only engine state, piggybacked on native responses so cached
            // Kotlin metadata does not require another worker round trip.
            writeln!(
                source,
                "__attribute__((export_name(\"reseam_bridge_revision\"))) jlong reseam_bridge_revision(void) {{ return {original}(&browser_env, NULL); }}"
            )?;
        }
        methods.push(BridgeMethod {
            buffer_lengths: buffer_lengths(method),
            export,
            name: if wide {
                format!("{original}_1browser")
            } else {
                original
            },
            parameters: parameters
                .iter()
                .map(|slot| slot.transported().signature())
                .collect(),
            returns: returns.transported().signature(),
        });
    }
    kotlin.push_str("}\n");
    let browser = output.join("browser");
    let java_path = browser.join("kotlin/app/reseam/patch/native");
    fs::create_dir_all(&java_path)?;
    fs::write(java_path.join("Native.kt"), kotlin)?;
    fs::write(browser.join("bridge.c"), source)?;
    fs::write(
        browser.join("methods.json"),
        serde_json::to_vec_pretty(&methods)?,
    )?;
    Ok(())
}

/// Maps each byte buffer argument to the argument carrying its length.
fn buffer_lengths(method: &NativeMethod) -> BTreeMap<usize, usize> {
    let mut lengths = BTreeMap::new();
    let mut argument = 0;
    for parameter in method.parameters() {
        if matches!(parameter.kind(), NativeParameterKind::Bytes(_)) {
            lengths.insert(argument, argument + 1);
            argument += 2;
        } else {
            argument += 1;
        }
    }
    lengths
}

fn c_method(export: &str, call: &str, parameters: &[Jni], returns: Jni) -> String {
    let declarations = parameters
        .iter()
        .enumerate()
        .map(|(index, slot)| format!("{} arg{index}", slot.transported().c_type()))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments =
        parameters
            .iter()
            .enumerate()
            .fold(String::new(), |mut output, (index, &slot)| {
                if slot == Jni::LONG {
                    write!(output, ", browser_long(arg{index})")
                } else {
                    write!(output, ", arg{index}")
                }
                .expect("writing to a String is infallible");
                output
            });
    let result = if returns == Jni::Void { "" } else { "return " };
    let call = if returns == Jni::LONG {
        format!("browser_pack_long({call}(&browser_env, NULL{arguments}))")
    } else {
        format!("{call}(&browser_env, NULL{arguments})")
    };
    format!(
        "__attribute__((export_name(\"{export}\"))) {} {export}({declarations}) {{ {result}{call}; }}\n",
        returns.transported().c_type()
    )
}
