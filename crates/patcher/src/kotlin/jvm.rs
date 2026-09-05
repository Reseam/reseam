// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The JVM that runs patches: the one the host process already lives in
//! (an Android app, a desktop JVM that loaded the engine through JNI), or a
//! desktop JVM started on demand for a plain native host such as the CLI.

use jni::objects::{JObject, JString, JThrowable, JValue};
use jni::{JNIEnv, JavaVM};

use crate::error::{PatcherError, Result};
use crate::JvmHeapStats;

pub(super) fn jvm_err(reason: impl std::fmt::Display) -> PatcherError {
    PatcherError::Jvm(reason.to_string())
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use jni::sys::{jint, jsize, JNI_OK};
    use jni::{InitArgsBuilder, JNIVersion, JavaVM};

    static JVM: OnceLock<std::result::Result<JavaVM, String>> = OnceLock::new();

    pub fn get_or_init() -> std::result::Result<&'static JavaVM, String> {
        JVM.get_or_init(init).as_ref().map_err(Clone::clone)
    }

    pub fn current() -> Option<&'static JavaVM> {
        JVM.get()?.as_ref().ok()
    }

    fn init() -> std::result::Result<JavaVM, String> {
        if let Some(vm) = running() {
            return Ok(vm);
        }
        let java_home = find_java_home().ok_or("JAVA_HOME not set and java not found on PATH")?;
        let jvm_lib = find_jvm_lib(&java_home)
            .ok_or_else(|| format!("libjvm not found in {}", java_home.display()))?;
        let jvm_dir = jvm_lib.parent().and_then(Path::to_str).unwrap_or_default();
        let ld_path = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        if !ld_path.split(':').any(|dir| dir == jvm_dir) {
            std::env::set_var("LD_LIBRARY_PATH", format!("{jvm_dir}:{ld_path}"));
        }
        let heap = std::env::var("RESEAM_JVM_HEAP").unwrap_or_else(|_| "256m".into());
        let args = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option(format!("-Xmx{heap}"))
            .option("-Xms16m")
            .option("-XX:+UseSerialGC")
            .option("-XX:MinHeapFreeRatio=10")
            .option("-XX:MaxHeapFreeRatio=30")
            .build()
            .map_err(|e| format!("JVM args: {e}"))?;
        JavaVM::new(args).map_err(|e| format!("JVM init: {e}"))
    }

    /// The JVM this process already runs in. A JVM host loads libjvm into
    /// the global symbol scope, so its invocation API is reachable from any
    /// library loaded afterwards.
    fn running() -> Option<JavaVM> {
        type GetCreatedJavaVms =
            unsafe extern "system" fn(*mut *mut jni::sys::JavaVM, jsize, *mut jsize) -> jint;
        let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"JNI_GetCreatedJavaVMs".as_ptr()) };
        if symbol.is_null() {
            return None;
        }
        let get_created: GetCreatedJavaVms = unsafe { std::mem::transmute(symbol) };
        let mut vm = std::ptr::null_mut();
        let mut count: jsize = 0;
        unsafe {
            if get_created(&mut vm, 1, &mut count) != JNI_OK || count == 0 || vm.is_null() {
                return None;
            }
            JavaVM::from_raw(vm).ok()
        }
    }

    fn find_java_home() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
        {
            return Some(home);
        }
        let output = std::process::Command::new("java")
            .args(["-XshowSettings:property", "-version"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("java.home")?
                    .split_once('=')
                    .map(|(_, v)| v.trim())
            })
            .map(PathBuf::from)
            .find(|path| path.is_dir())
    }

    fn find_jvm_lib(java_home: &Path) -> Option<PathBuf> {
        [
            "lib/server/libjvm.so",
            "lib/amd64/server/libjvm.so",
            "lib/client/libjvm.so",
            "jre/lib/server/libjvm.so",
            "jre/lib/amd64/server/libjvm.so",
            "lib/server/libjvm.dylib",
            "lib/libjvm.dylib",
        ]
        .iter()
        .map(|candidate| java_home.join(candidate))
        .find(|path| path.exists())
    }
}

pub(super) fn get_or_init() -> Result<&'static JavaVM> {
    #[cfg(not(target_os = "android"))]
    {
        desktop::get_or_init().map_err(jvm_err)
    }
    #[cfg(target_os = "android")]
    {
        super::android_host::java_vm().map_err(jvm_err)
    }
}

/// The running JVM without starting one.
fn current() -> Option<&'static JavaVM> {
    #[cfg(not(target_os = "android"))]
    {
        desktop::current()
    }
    #[cfg(target_os = "android")]
    {
        super::android_host::java_vm().ok()
    }
}

/// Runs JNI work in its own local frame so the references it creates die
/// with the frame instead of pinning objects for the thread's lifetime.
/// A failure never leaves a Java exception pending: the next JNI call would
/// abort the process under CheckJNI. The exception's trace joins the error.
pub(super) fn with_frame<T>(
    env: &mut JNIEnv<'_>,
    f: impl FnOnce(&mut JNIEnv<'_>) -> Result<T>,
) -> Result<T> {
    enum FrameError {
        Patch(PatcherError),
        Jni(jni::errors::Error),
    }
    impl From<jni::errors::Error> for FrameError {
        fn from(e: jni::errors::Error) -> Self {
            Self::Jni(e)
        }
    }
    env.with_local_frame(64, |env| f(env).map_err(FrameError::Patch))
        .map_err(|e| {
            let error = match e {
                FrameError::Patch(e) => e,
                FrameError::Jni(e) => jvm_err(format!("local frame: {e}")),
            };
            match (error, take_pending_exception(env)) {
                (PatcherError::Jvm(message), Some(trace)) => {
                    PatcherError::Jvm(format!("{message}: {trace}"))
                }
                (error, Some(trace)) => jvm_err(format!("{error}: {trace}")),
                (error, None) => error,
            }
        })
}

/// Runs a full collection so the run's class loader, its patch objects, and
/// the heap they inflated are released before the host measures or reuses
/// the process.
pub(crate) fn collect_garbage() {
    let Some(vm) = current() else {
        return;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return;
    };
    let _ = with_frame(&mut env, |env| {
        let _ = env.call_static_method("java/lang/System", "gc", "()V", &[]);
        clear_pending_exception(env);
        Ok(())
    });
}

pub(crate) fn heap_stats() -> Option<JvmHeapStats> {
    let vm = current()?;
    let mut env = vm.attach_current_thread().ok()?;
    with_frame(&mut env, |env| {
        let runtime = env
            .call_static_method(
                "java/lang/Runtime",
                "getRuntime",
                "()Ljava/lang/Runtime;",
                &[],
            )
            .and_then(|v| v.l())
            .map_err(|e| jvm_err(format!("Runtime.getRuntime: {e}")))?;
        let mut long = |name: &str| {
            env.call_method(&runtime, name, "()J", &[])
                .and_then(|v| v.j())
                .map(|v| v as u64)
                .map_err(|e| jvm_err(format!("Runtime.{name}: {e}")))
        };
        let total = long("totalMemory")?;
        let free = long("freeMemory")?;
        let max = long("maxMemory")?;
        Ok(JvmHeapStats {
            used_bytes: total.saturating_sub(free),
            committed_bytes: total,
            max_bytes: max,
        })
    })
    .ok()
}

/// The pending Java exception, cleared and rendered with its stack trace.
pub(super) fn take_pending_exception(env: &mut JNIEnv<'_>) -> Option<String> {
    if !env.exception_check().unwrap_or(false) {
        return None;
    }
    let throwable = env.exception_occurred().ok();
    env.exception_clear().ok();
    Some(match throwable {
        Some(throwable) => describe_throwable(env, &throwable),
        None => "Java exception was thrown; failed to read throwable".to_string(),
    })
}

fn describe_throwable(env: &mut JNIEnv<'_>, throwable: &JThrowable<'_>) -> String {
    let described = (|| -> jni::errors::Result<String> {
        let writer = env.new_object("java/io/StringWriter", "()V", &[])?;
        let print_writer = env.new_object(
            "java/io/PrintWriter",
            "(Ljava/io/Writer;)V",
            &[JValue::Object(&writer)],
        )?;
        env.call_method(
            throwable,
            "printStackTrace",
            "(Ljava/io/PrintWriter;)V",
            &[JValue::Object(&print_writer)],
        )?;
        env.call_method(&print_writer, "flush", "()V", &[])?;
        let text = env
            .call_method(&writer, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string(&JString::from(text))?.into())
    })();
    clear_pending_exception(env);
    match described {
        Ok(trace) if !trace.trim().is_empty() => trace,
        _ => "Java exception was thrown".to_string(),
    }
}

pub(super) fn clear_pending_exception(env: &mut JNIEnv<'_>) {
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
    }
}

pub(super) fn string_of(env: &mut JNIEnv<'_>, value: JObject<'_>) -> jni::errors::Result<String> {
    Ok(env.get_string(&JString::from(value))?.into())
}
