// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

/// Splits the `BoltFFI` Kotlin output into internal value types and checked bridge calls.
pub fn split_surface(output: &Path) -> Result<()> {
    let path = output.join("app/reseam/patch/native/ReseamPatcher.kt");
    let source = fs::read_to_string(&path)?;
    let native = source
        .find("@Suppress(\"FunctionName\")\nprivate object Native {")
        .context("generated Kotlin has no hosted Native object")?;
    let native_end = source[native..]
        .find("\n}\n")
        .map(|end| native + end + 3)
        .context("generated Native object is not closed")?;
    let first_function = source[native_end..]
        .find("\nfun ")
        .map(|start| native_end + start)
        .context("generated Kotlin has no callable functions")?;
    let values_end = source[..first_function]
        .rfind("\n}\n")
        .map(|end| end + 3)
        .context("generated Kotlin values are not closed")?;
    let exceptions = source
        .lines()
        .filter(|line| {
            line.starts_with("class FfiException(")
                || line.starts_with("internal class BoltFfiErrorBufferException(")
        })
        .map(|line| {
            if line.starts_with("class ") {
                format!("internal {line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prelude = source[..native]
        .lines()
        .filter(|line| {
            !line.starts_with("class FfiException(")
                && !line.starts_with("internal class BoltFfiErrorBufferException(")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut values = prelude.replace(
        "package app.reseam.patch.native",
        "package app.reseam.patch.types",
    );
    values = values
        .lines()
        .map(|line| {
            line.strip_prefix("private ")
                .map_or_else(|| line.to_owned(), |rest| format!("internal {rest}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    values = values.replace(
        "package app.reseam.patch.types",
        "package app.reseam.patch.types\n\nimport app.reseam.patch.native.FfiException",
    );
    values.push_str(&source[native_end..values_end]);
    values = owned_kotlin_types(&values);
    for name in [
        "CodeInstruction",
        "CodeOperand",
        "FieldPattern",
        "InstructionHit",
        "MethodEdit",
        "MethodPattern",
        "RegisterWriters",
        "ResourceScalar",
    ] {
        values = values.replace(
            &format!("\ndata class {name}("),
            &format!("\ninternal data class {name}("),
        );
    }
    for name in ["CodeEdit", "CodePlacement", "CodeRegister", "CodeStep"] {
        values = values.replace(
            &format!("\nsealed class {name} {{"),
            &format!("\ninternal sealed class {name} {{"),
        );
    }
    let values_path = output.join("app/reseam/patch/types");
    fs::create_dir_all(&values_path)?;
    fs::write(values_path.join("PatchValues.kt"), values)?;
    let calls = checked_kotlin_calls(&owned_kotlin_types(&source[values_end..]))?;

    fs::write(
        path,
        format!(
            "@file:OptIn(kotlin.ExperimentalUnsignedTypes::class)\npackage app.reseam.patch.native\n\nimport app.reseam.patch.types.*\n\n{}\n{exceptions}\n{calls}\nprivate inline fun <T> bridgeCall(block: () -> T): T =\n    try {{ block() }} catch (error: BoltFfiErrorBufferException) {{\n        throw FfiException(WireReader(error.bytes).readString())\n    }}\n",
            &source[native..native_end]
        ),
    )?;
    Ok(())
}

fn checked_kotlin_calls(source: &str) -> Result<String> {
    let mut wrapped = false;
    let mut result = String::new();
    for line in source.lines() {
        let rendered = if let Some(declaration) = line.strip_prefix("fun ") {
            let declaration = declaration
                .strip_suffix(" {")
                .context("generated Kotlin callable must have a braced body")?;
            let (name, _) = declaration
                .split_once('(')
                .context("generated Kotlin callable must have parameters")?;
            wrapped = !matches!(name, "checkCall" | "checkInvocation");
            format!(
                "internal fun {declaration}{} {{",
                if wrapped { " = bridgeCall" } else { "" }
            )
        } else if wrapped && line == "}" {
            wrapped = false;
            line.to_owned()
        } else if wrapped && line.trim_start().starts_with("return") {
            line.replacen("return", "return@bridgeCall", 1)
        } else {
            line.to_owned()
        };
        result.push_str(&rendered);
        result.push('\n');
    }
    ensure!(!wrapped, "generated Kotlin callable is not closed");
    Ok(result)
}

fn owned_kotlin_types(source: &str) -> String {
    source
        .replace("app.reseam.patch.native.", "app.reseam.patch.types.")
        .replace(
            "app.reseam.patch.types.FfiException",
            "app.reseam.patch.native.FfiException",
        )
        .replace(
            "app.reseam.patch.types.BoltFfiErrorBufferException",
            "app.reseam.patch.native.BoltFfiErrorBufferException",
        )
}
