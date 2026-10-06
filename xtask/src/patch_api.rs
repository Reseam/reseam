// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
    browser_bridge(&contract, &output)?;
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

fn browser_bridge(contract: &JniBridgeContract, output: &std::path::Path) -> Result<()> {
    let mut source = String::from(
        "/* Generated from BoltFFI Binding IR. Do not edit. */\n#include \"transport.c\"\n#include \"registration.c\"\n",
    );
    let mut methods = Vec::new();
    // LiveConnect converts scalar Java longs through JavaScript Number. Arrays
    // cross CheerpJ by reference, preserving every bit in either direction.
    // Keeping the current revision in Java also avoids a LiveConnect call for
    // each metadata cache check; native responses update the shared long array.
    let mut kotlin = String::from(
        "// Generated from BoltFFI Binding IR. Do not edit.\npackage app.reseam.patch.native\ninternal object Native {\n    private val browserRevision = ThreadLocal<LongArray>()\n    @JvmStatic fun beginBrowserInvocation(value: LongArray) { check(browserRevision.get() == null); browserRevision.set(value) }\n    @JvmStatic fun endBrowserInvocation() { browserRevision.remove() }\n    fun ensureInitialized() {}\n",
    );
    for (index, method) in contract.methods().iter().enumerate() {
        let parameters = method
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
                    other => bail!("unsupported browser bridge parameter: {other:?}"),
                })
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut buffer_lengths = std::collections::BTreeMap::new();
        let mut argument_index = 0;
        for parameter in method.parameters() {
            if matches!(parameter.kind(), NativeParameterKind::Bytes(_)) {
                buffer_lengths.insert(argument_index, argument_index + 1);
                argument_index += 2;
            } else {
                argument_index += 1;
            }
        }
        let returns = return_signature(method.returns())?;
        let wide = returns == "J" || parameters.contains(&"J");
        let transported = parameters
            .iter()
            .map(|&ty| if ty == "J" { "[J" } else { ty })
            .collect::<Vec<_>>();
        let transported_return = if returns == "J" { "[J" } else { returns };
        browser_kotlin_method(
            &mut kotlin,
            method.c_function().name(),
            &parameters,
            returns,
        )?;
        let name = format!("reseam_bridge_{index}");
        let original = method.symbol().to_string();
        let exempt = [
            "_ctx_is_active",
            "_check_call",
            "_invoke_scratch_words",
            "_lower_instruction",
            "_lower_instructions",
        ]
        .iter()
        .any(|suffix| method.c_function().name().ends_with(suffix));
        let call = if exempt {
            original.clone()
        } else {
            format!("reseam_checked_{index}")
        };
        source.push_str(&browser_c_method(&name, &call, &parameters, returns)?);
        if method
            .c_function()
            .name()
            .ends_with("_handles_mutation_revision")
        {
            // Read-only engine state, piggybacked on native responses so cached
            // Kotlin metadata does not require another worker round trip.
            writeln!(
                source,
                "__attribute__((export_name(\"reseam_bridge_revision\"))) jlong reseam_bridge_revision(void) {{ return {original}(&browser_env, NULL); }}"
            )?;
        }
        let symbol = if wide {
            format!("{original}_1browser")
        } else {
            original
        };
        methods.push(serde_json::json!({"name": symbol, "export": name, "parameters": transported, "bufferLengths": buffer_lengths, "returns": transported_return}));
    }
    let browser = output.join("browser");
    fs::create_dir_all(&browser)?;
    kotlin.push_str("}\n");
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

fn browser_c_method(name: &str, call: &str, parameters: &[&str], returns: &str) -> Result<String> {
    let transported = parameters
        .iter()
        .map(|&ty| if ty == "J" { "[J" } else { ty })
        .collect::<Vec<_>>();
    let transported_return = if returns == "J" { "[J" } else { returns };
    let declarations = transported
        .iter()
        .enumerate()
        .map(|(i, signature)| Ok(format!("{} arg{i}", jni_c_type(signature)?)))
        .collect::<Result<Vec<_>>>()?
        .join(", ");
    let arguments = (0..parameters.len()).fold(String::new(), |mut output, index| {
        if parameters[index] == "J" {
            write!(output, ", browser_long(arg{index})")
                .expect("writing to a String is infallible");
        } else {
            write!(output, ", arg{index}").expect("writing to a String is infallible");
        }
        output
    });
    let result = if returns == "V" { "" } else { "return " };
    let pack = if returns == "J" {
        "browser_pack_long("
    } else {
        ""
    };
    let end_pack = if returns == "J" { ")" } else { "" };
    Ok(format!(
        "__attribute__((export_name(\"{name}\"))) {} {name}({declarations}) {{ {result}{pack}{call}(&browser_env, NULL{arguments}){end_pack}; }}\n",
        jni_c_type(transported_return)?
    ))
}

fn browser_kotlin_method(
    kotlin: &mut String,
    java_name: &str,
    parameters: &[&str],
    returns: &str,
) -> Result<()> {
    if java_name.ends_with("_handles_mutation_revision") {
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
    let wide = returns == "J" || parameters.contains(&"J");
    let native_name = if wide {
        format!("{java_name}_browser")
    } else {
        java_name.to_string()
    };
    let java_parameters = parameters
        .iter()
        .enumerate()
        .map(|(i, ty)| Ok(format!("arg{i}: {}", browser_kotlin_type(ty)?)))
        .collect::<Result<Vec<_>>>()?
        .join(", ");
    if wide {
        let args = parameters
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                if *ty == "J" {
                    format!("longArrayOf(arg{i})")
                } else {
                    format!("arg{i}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let result = if returns == "V" { "" } else { "return " };
        let unwrap = if returns == "J" { "[0]" } else { "" };
        writeln!(
            kotlin,
            "    @JvmStatic fun {java_name}({java_parameters}): {} {{ {result}{native_name}({args}){unwrap} }}",
            browser_kotlin_type(returns)?
        )?;
    }
    let native_parameters = parameters
        .iter()
        .map(|&ty| if ty == "J" { "[J" } else { ty })
        .collect::<Vec<_>>()
        .iter()
        .enumerate()
        .map(|(i, ty)| Ok(format!("arg{i}: {}", browser_kotlin_type(ty)?)))
        .collect::<Result<Vec<_>>>()?
        .join(", ");
    writeln!(
        kotlin,
        "    @JvmStatic external fun {native_name}({native_parameters}): {}",
        browser_kotlin_type(if returns == "J" { "[J" } else { returns })?
    )?;
    Ok(())
}

fn browser_kotlin_type(signature: &str) -> Result<&'static str> {
    Ok(match signature {
        "V" => "Unit",
        "Z" => "Boolean",
        "B" => "Byte",
        "C" => "Char",
        "S" => "Short",
        "I" => "Int",
        "J" => "Long",
        "F" => "Float",
        "D" => "Double",
        "[B" => "ByteArray?",
        "[S" => "ShortArray",
        "[I" => "IntArray",
        "[J" => "LongArray",
        "Ljava/nio/ByteBuffer;" => "java.nio.ByteBuffer",
        other => bail!("unsupported browser Java type: {other}"),
    })
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
