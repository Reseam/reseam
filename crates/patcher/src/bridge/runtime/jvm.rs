// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use jni::objects::{JThrowable, JValue, JValueOwned};

use super::jni_values::string_of;
use jni::{Env, JavaVM, jni_sig, jni_str};

use crate::JvmHeapStats;
use crate::error::{PatcherError, Result};

impl From<jni::errors::Error> for PatcherError {
    fn from(error: jni::errors::Error) -> Self {
        jvm_err(error)
    }
}

pub(in crate::bridge) fn jvm_err(reason: impl std::fmt::Display) -> PatcherError {
    PatcherError::Jvm(reason.to_string())
}

pub(in crate::bridge) fn get_or_init() -> Result<&'static JavaVM> {
    #[cfg(not(target_os = "android"))]
    {
        super::desktop::get_or_init()
    }
    #[cfg(target_os = "android")]
    {
        super::android_host::java_vm()
    }
}

fn current() -> Option<&'static JavaVM> {
    #[cfg(not(target_os = "android"))]
    {
        super::desktop::current()
    }
    #[cfg(target_os = "android")]
    {
        super::android_host::java_vm().ok()
    }
}

pub(in crate::bridge) fn with_frame<T>(
    env: &mut Env<'_>,
    f: impl FnOnce(&mut Env<'_>) -> Result<T>,
) -> Result<T> {
    let result = env.with_local_frame(64, |env| {
        let result = f(env);
        let exception = take_pending_exception(env);
        match (result, exception) {
            (Ok(value), Ok(None)) => Ok(value),
            (Err(error), Ok(None)) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Ok(Some(trace))) => Err(jvm_err(format!("{error}: {trace}"))),
            (Ok(_), Ok(Some(trace))) => Err(jvm_err(trace)),
            (Err(error), Err(trace_error)) => Err(jvm_err(format!("{error}; {trace_error}"))),
        }
    });
    match result {
        Err(error) => Err(match take_pending_exception(env)? {
            Some(trace) => jvm_err(format!("{error}: {trace}")),
            None => error,
        }),
        result => result,
    }
}

pub(crate) fn collect_garbage() {
    let Some(vm) = current() else { return };
    let collected = vm.attach_current_thread(|env| {
        with_frame(env, |env| {
            env.call_static_method(
                jni_str!("java/lang/System"),
                jni_str!("gc"),
                jni_sig!("()V"),
                &[],
            )?;
            Ok(())
        })
    });
    if let Err(error) = collected {
        tracing::warn!(%error, "failed to collect patch runtime garbage");
    }
}

pub(crate) fn heap_stats() -> Option<JvmHeapStats> {
    let vm = current()?;
    vm.attach_current_thread(|env| {
        with_frame(env, |env| {
            let runtime = env
                .call_static_method(
                    jni_str!("java/lang/Runtime"),
                    jni_str!("getRuntime"),
                    jni_sig!("()Ljava/lang/Runtime;"),
                    &[],
                )
                .and_then(JValueOwned::l)
                .map_err(|e| jvm_err(format!("Runtime.getRuntime: {e}")))?;
            let mut long = |name: &str| {
                env.call_method(
                    &runtime,
                    jni::strings::JNIString::new(name),
                    jni_sig!("()J"),
                    &[],
                )
                .and_then(JValueOwned::j)
                .map(|v| v as u64)
                .map_err(|e| jvm_err(format!("Runtime.{name}: {e}")))
            };
            let total = long("totalMemory")?;
            let free = long("freeMemory")?;
            let max = long("maxMemory")?;
            Ok(JvmHeapStats {
                used: total.saturating_sub(free),
                committed: total,
                max,
            })
        })
    })
    .ok()
}

/// The result of a Java call, failing with the exception it threw if any.
pub(in crate::bridge) fn outcome<T>(
    env: &mut Env<'_>,
    call: jni::errors::Result<T>,
    what: &str,
) -> Result<T> {
    match (call, take_pending_exception(env)?) {
        (_, Some(exception)) => Err(jvm_err(format!("{what}: {exception}"))),
        (Err(error), None) => Err(jvm_err(format!("{what}: {error}"))),
        (Ok(value), None) => Ok(value),
    }
}

fn take_pending_exception(env: &mut Env<'_>) -> Result<Option<String>> {
    let Some(throwable) = env.exception_occurred() else {
        return Ok(None);
    };
    env.exception_clear();
    let described = env.with_local_frame(16, |env| describe_throwable(env, &throwable));
    if described.is_err() {
        env.exception_clear();
    }
    described
        .map(Some)
        .map_err(|error| jvm_err(format!("describe Java exception: {error}")))
}

fn describe_throwable(
    env: &mut Env<'_>,
    throwable: &JThrowable<'_>,
) -> jni::errors::Result<String> {
    let writer = env.new_object(jni_str!("java/io/StringWriter"), jni_sig!("()V"), &[])?;
    let print_writer = env.new_object(
        jni_str!("java/io/PrintWriter"),
        jni_sig!("(Ljava/io/Writer;)V"),
        &[JValue::Object(&writer)],
    )?;
    env.call_method(
        throwable,
        jni_str!("printStackTrace"),
        jni_sig!("(Ljava/io/PrintWriter;)V"),
        &[JValue::Object(&print_writer)],
    )?;
    env.call_method(&print_writer, jni_str!("flush"), jni_sig!("()V"), &[])?;
    let text = env
        .call_method(
            &writer,
            jni_str!("toString"),
            jni_sig!("()Ljava/lang/String;"),
            &[],
        )?
        .l()?;
    string_of(env, text)
}
