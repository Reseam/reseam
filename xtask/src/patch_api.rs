// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::opcodes::opcode_metadata;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;

use anyhow::{Context, Result, bail, ensure};
use boltffi_backend::bridge::jni::{JniBridgeContract, NativeParameterKind, NativeReturn};
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
    Generation::write_output(target.render(&bindings)?, &output)?;
    fs::write(output.join("jni/registration.c"), registration(&contract)?)?;
    kotlin_surface(&output)?;
    let opcodes = output.join("app/reseam/patch/dex");
    fs::create_dir_all(&opcodes)?;
    let (widths, opcode) = opcode_metadata();
    fs::write(opcodes.join("OpcodeWidths.kt"), widths)?;
    fs::write(opcodes.join("Opcode.kt"), opcode)?;
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
    let check = contract
        .methods()
        .iter()
        .find(|method| method.c_function().name().ends_with("_check_call"))
        .context("patch bridge must export check_call")?;
    ensure!(
        check.parameters().is_empty() && return_signature(check.returns())? == "V",
        "check_call must have no arguments or return carrier"
    );
    let mut wrappers = String::new();
    let mut entries = Vec::new();
    for (index, method) in contract.methods().iter().enumerate() {
        let signatures = method
            .parameters()
            .iter()
            .map(|parameter| {
                Ok(match parameter.kind() {
                    NativeParameterKind::Scalar(value) => vec![value.ty().signature()],
                    NativeParameterKind::Bytes(_) => vec!["Ljava/nio/ByteBuffer;", "I"],
                    NativeParameterKind::Record(_) => vec!["Ljava/nio/ByteBuffer;"],
                    NativeParameterKind::DirectVector(value) => {
                        vec![value.jni_type().array_signature()]
                    }
                    other => bail!("unsupported hosted JNI parameter: {other:?}"),
                })
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let returns = return_signature(method.returns())?;
        let symbol = if [
            "_ctx_is_active",
            "_check_call",
            "_invoke_scratch_words",
            "_lower_instruction",
            "_lower_instructions",
        ]
        .iter()
        .any(|name| method.c_function().name().ends_with(name))
        {
            method.symbol().to_string()
        } else {
            let name = format!("reseam_checked_{index}");
            wrappers.push_str(&checked_jni(
                &name,
                &method.symbol().to_string(),
                &check.symbol().to_string(),
                &signatures,
                returns,
            )?);
            name
        };
        entries.push(format!(
            "    {{\"{}\", \"({}){returns}\", (void *){symbol}}},",
            method.c_function().name(),
            signatures.join("")
        ));
    }
    let entries = entries.join("\n");
    Ok(format!(
        r#"/* Generated from BoltFFI Binding IR. Do not edit. */
#include "jni_glue.c"

extern int32_t reseam_patch_call_failed(void);
{wrappers}
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

fn checked_jni(
    name: &str,
    original: &str,
    check: &str,
    parameters: &[&str],
    returns: &str,
) -> Result<String> {
    let declarations = parameters
        .iter()
        .enumerate()
        .map(|(index, signature)| Ok(format!(", {} arg{index}", jni_c_type(signature)?)))
        .collect::<Result<String>>()?;
    let arguments = (0..parameters.len()).fold(String::new(), |mut output, index| {
        write!(output, ", arg{index}").expect("writing to a String is infallible");
        output
    });
    let result = if returns == "V" {
        String::new()
    } else {
        format!("{} value = ", jni_c_type(returns)?)
    };
    let failure = if returns == "V" {
        "return;"
    } else {
        "return 0;"
    };
    let success = if returns == "V" {
        "return;"
    } else {
        "return value;"
    };
    Ok(format!(
        r"static {} JNICALL {name}(JNIEnv *env, jclass klass{declarations}) {{
    if (reseam_patch_call_failed()) {{
        {check}(env, klass);
        {failure}
    }}
    {result}{original}(env, klass{arguments});
    if (reseam_patch_call_failed() && !(*env)->ExceptionCheck(env)) {{
        {check}(env, klass);
        {failure}
    }}
    {success}
}}
",
        jni_c_type(returns)?
    ))
}

fn jni_c_type(signature: &str) -> Result<&'static str> {
    Ok(match signature {
        "V" => "void",
        "Z" => "jboolean",
        "B" => "jbyte",
        "C" => "jchar",
        "S" => "jshort",
        "I" => "jint",
        "J" => "jlong",
        "F" => "jfloat",
        "D" => "jdouble",
        "[Z" => "jbooleanArray",
        "[B" => "jbyteArray",
        "[C" => "jcharArray",
        "[S" => "jshortArray",
        "[I" => "jintArray",
        "[J" => "jlongArray",
        "[F" => "jfloatArray",
        "[D" => "jdoubleArray",
        "Ljava/nio/ByteBuffer;" => "jobject",
        other => bail!("unsupported JNI type: {other}"),
    })
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

fn kotlin_surface(output: &std::path::Path) -> Result<()> {
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
        "FingerprintDef",
        "FingerprintResult",
        "InstructionHit",
        "RegisterWriters",
        "MethodCallSiteResult",
        "ResourceScalar",
        "ScratchSpan",
        "MethodEdit",
    ] {
        values = values.replace(
            &format!("\ndata class {name}("),
            &format!("\ninternal data class {name}("),
        );
    }
    let values_path = output.join("app/reseam/patch/types");
    fs::create_dir_all(&values_path)?;
    let (imports, aliases) = kotlin_value_aliases(&values);
    fs::write(values_path.join("PatchValues.kt"), values)?;
    let calls = checked_kotlin_calls(&owned_kotlin_types(&source[values_end..]))?;

    fs::write(
        path,
        format!(
            "@file:OptIn(kotlin.ExperimentalUnsignedTypes::class)\npackage app.reseam.patch.native\n\nimport app.reseam.patch.types.*\n{imports}\n{}\n{exceptions}\n{calls}\nprivate inline fun <T> bridgeCall(block: () -> T): T =\n    try {{ block() }} catch (error: BoltFfiErrorBufferException) {{\n        throw FfiException(WireReader(error.bytes).readString())\n    }}\n",
            &source[native..native_end]
        ),
    )?;
    fs::write(
        output.join("app/reseam/patch/native/References.kt"),
        format!("package app.reseam.patch.native\n\n{aliases}"),
    )?;
    Ok(())
}

fn kotlin_value_aliases(source: &str) -> (String, String) {
    let names: Vec<_> = source
        .lines()
        .filter_map(|line| {
            ["data class ", "sealed class ", "enum class "]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))?
                .split(['(', '{', ':', ' '])
                .next()
        })
        .collect();
    let (mut imports, mut aliases) = (String::new(), String::new());
    for name in names {
        writeln!(imports, "import app.reseam.patch.types.{name}")
            .expect("writing to a String is infallible");
        writeln!(aliases,
            "@Deprecated(\"Import app.reseam.patch.types.{name}\", ReplaceWith(\"{name}\", \"app.reseam.patch.types.{name}\"))\ntypealias {name} = app.reseam.patch.types.{name}\n"
        ).expect("writing to a String is infallible");
    }
    (imports, aliases)
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
            wrapped = !matches!(name, "ctxIsActive" | "checkCall" | "checkInvocation");
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
