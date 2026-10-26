// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Write as _;

use anyhow::{Context, Result, ensure};
use boltffi_backend::bridge::jni::JniBridgeContract;

use super::jni::{self, Jni};

/// C source registering every native method, with a failure check around each phase callback.
pub fn source(contract: &JniBridgeContract) -> Result<String> {
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
        check.parameters().is_empty() && jni::returns(check.returns())? == Jni::Void,
        "check_call must have no arguments or return carrier"
    );
    let check = check.symbol().to_string();
    let mut wrappers = String::new();
    let mut entries = Vec::new();
    for (index, method) in contract.methods().iter().enumerate() {
        let parameters = jni::parameters(method)?;
        let returns = jni::returns(method.returns())?;
        let symbol = if jni::is_checked(method) {
            let name = checked_name(index);
            wrappers.push_str(&checked(
                &name,
                &method.symbol().to_string(),
                &check,
                &parameters,
                returns,
            ));
            name
        } else {
            method.symbol().to_string()
        };
        entries.push(format!(
            "    {{\"{}\", \"{}\", (void *){symbol}}},",
            method.c_function().name(),
            jni::descriptor(&parameters, returns)
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

/// The registered symbol of a checked method; the browser bridge calls the same wrapper.
pub fn checked_name(index: usize) -> String {
    format!("reseam_checked_{index}")
}

fn checked(name: &str, original: &str, check: &str, parameters: &[Jni], returns: Jni) -> String {
    let declarations =
        parameters
            .iter()
            .enumerate()
            .fold(String::new(), |mut output, (index, slot)| {
                write!(output, ", {} arg{index}", slot.c_type())
                    .expect("writing to a String is infallible");
                output
            });
    let arguments = (0..parameters.len()).fold(String::new(), |mut output, index| {
        write!(output, ", arg{index}").expect("writing to a String is infallible");
        output
    });
    let void = returns == Jni::Void;
    let result = if void {
        String::new()
    } else {
        format!("{} value = ", returns.c_type())
    };
    let failure = if void { "return;" } else { "return 0;" };
    let success = if void { "return;" } else { "return value;" };
    format!(
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
        returns.c_type()
    )
}
