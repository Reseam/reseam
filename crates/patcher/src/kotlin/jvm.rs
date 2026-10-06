// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use jni::objects::{JObject, JString, JThrowable, JValue, JValueOwned};
use jni::{Env, JavaVM, jni_sig, jni_str};

use crate::JvmHeapStats;
use crate::error::{PatcherError, Result};

impl From<jni::errors::Error> for PatcherError {
    fn from(error: jni::errors::Error) -> Self {
        jvm_err(error)
    }
}

pub(super) fn jvm_err(reason: impl std::fmt::Display) -> PatcherError {
    PatcherError::Jvm(reason.to_string())
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use jni::sys::{JNI_OK, jint, jsize};
    use jni::{InitArgsBuilder, JNIVersion, JavaVM};

    use std::fs::File;
    use std::io::Write;

    use sha2::{Digest, Sha256};

    use super::jvm_err;
    use crate::error::Result;

    static JVM: OnceLock<Result<JavaVM>> = OnceLock::new();
    #[expect(
        clippy::large_include_file,
        reason = "desktop hosts embed one shared patch runtime"
    )]
    const RUNTIME: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.jar"));

    pub(super) fn get_or_init() -> Result<&'static JavaVM> {
        JVM.get_or_init(init).as_ref().map_err(jvm_err)
    }

    pub(super) fn current() -> Option<&'static JavaVM> {
        JVM.get()?.as_ref().ok()
    }

    fn runtime_jar() -> Result<PathBuf> {
        let root = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        }
        .unwrap_or_else(std::env::temp_dir);
        let digest: [u8; 32] = Sha256::digest(RUNTIME).into();
        // The digest also distinguishes unreleased builds of the same version.
        let cache = root
            .join("reseam")
            .join(crate::bundle::ENGINE_VERSION)
            .join(hex::encode(digest));
        std::fs::create_dir_all(&cache)?;
        let path = cache.join("runtime.jar");
        if !path.try_exists()? {
            let mut temporary = tempfile::NamedTempFile::new_in(&cache)?;
            temporary.write_all(RUNTIME)?;
            temporary.as_file().sync_all()?;
            match temporary.persist_noclobber(&path) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.error.into()),
            }
        }
        let actual = crate::bundle::copy_hashed(&mut File::open(&path)?, &mut std::io::sink())?;
        if actual != digest {
            return Err(jvm_err(format!(
                "cached patch runtime {} is corrupt",
                path.display()
            )));
        }
        Ok(path)
    }

    fn init() -> Result<JavaVM> {
        if let Some(vm) = running()? {
            return Ok(vm);
        }
        let java_home = find_java_home()?;
        let jvm_lib = find_jvm_lib(&java_home)
            .ok_or_else(|| jvm_err(format!("JVM library not found in {}", java_home.display())))?;
        let runtime = runtime_jar()?;
        let classpath = runtime
            .to_str()
            .ok_or_else(|| jvm_err("runtime cache path is not Unicode"))?;
        let heap = std::env::var("RESEAM_JVM_HEAP").unwrap_or_else(|_| "256m".into());
        let args = InitArgsBuilder::new()
            .version(JNIVersion::V1_8)
            .option(format!("-Djava.class.path={classpath}"))
            .option(format!("-Xmx{heap}"))
            .option("-Xms16m")
            .option("-XX:+UseSerialGC")
            // Patch code runs once per session: C2's optimizing compiles cost
            // about 20 MB of compiler memory and saved no time.
            .option("-XX:TieredStopAtLevel=1")
            .option("-XX:MinHeapFreeRatio=10")
            .option("-XX:MaxHeapFreeRatio=30")
            .build()
            .map_err(|error| jvm_err(format!("JVM args: {error}")))?;
        JavaVM::with_libjvm(args, || Ok(jvm_lib))
            .map_err(|error| jvm_err(format!("JVM init: {error}")))
    }

    fn running() -> Result<Option<JavaVM>> {
        type GetCreatedJavaVms =
            unsafe extern "system" fn(*mut *mut jni::sys::JavaVM, jsize, *mut jsize) -> jint;
        #[cfg(unix)]
        let host: libloading::Library = libloading::os::unix::Library::this().into();
        #[cfg(windows)]
        let host: libloading::Library = {
            let Ok(host) = libloading::os::windows::Library::open_already_loaded("jvm.dll") else {
                return Ok(None);
            };
            host.into()
        };
        // A native host has no invocation entry point until libjvm is loaded.
        // SAFETY: the symbol is the JNI invocation entry point with this signature.
        let Ok(get_created) =
            (unsafe { host.get::<GetCreatedJavaVms>(b"JNI_GetCreatedJavaVMs\0") })
        else {
            return Ok(None);
        };
        let mut vm = std::ptr::null_mut();
        let mut count: jsize = 0;
        // SAFETY: the invocation API writes at most one VM pointer and one count.
        let status = unsafe { get_created(&raw mut vm, 1, &raw mut count) };
        if status != JNI_OK {
            return Err(jvm_err(format!("JNI_GetCreatedJavaVMs failed: {status}")));
        }
        if count == 0 {
            return Ok(None);
        }
        // SAFETY: a successful invocation returned an existing VM owned by the host.
        Ok(Some(unsafe { JavaVM::from_raw(vm) }))
    }

    fn find_java_home() -> Result<PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = PathBuf::from(home);
            if !home.is_dir() {
                return Err(jvm_err(format!(
                    "JAVA_HOME is not a directory: {}",
                    home.display()
                )));
            }
            return Ok(home);
        }
        let output = std::process::Command::new("java")
            .args(["-XshowSettings:property", "-version"])
            .output()
            .map_err(|error| jvm_err(format!("locate java on PATH: {error}")))?;
        if !output.status.success() {
            return Err(jvm_err(format!(
                "java -version failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("java.home")?
                    .split_once('=')
                    .map(|(_, value)| PathBuf::from(value.trim()))
            })
            .find(|path| path.is_dir())
            .ok_or_else(|| jvm_err("java did not report a valid java.home"))
    }

    fn find_jvm_lib(java_home: &Path) -> Option<PathBuf> {
        const DIRS: &[&str] = if cfg!(windows) {
            &[
                "bin/server",
                "bin/client",
                "jre/bin/server",
                "jre/bin/client",
            ]
        } else {
            &[
                "lib/server",
                "lib/amd64/server",
                "lib/client",
                "jre/lib/server",
                "jre/lib/amd64/server",
                "lib",
            ]
        };
        let name = format!("{DLL_PREFIX}jvm{DLL_SUFFIX}");
        DIRS.iter()
            .map(|dir| java_home.join(dir).join(&name))
            .find(|path| path.exists())
    }
}

pub(super) fn get_or_init() -> Result<&'static JavaVM> {
    #[cfg(not(target_os = "android"))]
    {
        desktop::get_or_init()
    }
    #[cfg(target_os = "android")]
    {
        super::android_host::java_vm()
    }
}

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

pub(super) fn with_frame<T>(
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

pub(super) fn take_pending_exception(env: &mut Env<'_>) -> Result<Option<String>> {
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

pub(super) fn string_of(env: &mut Env<'_>, value: JObject<'_>) -> jni::errors::Result<String> {
    env.cast_local::<JString<'_>>(value)?.try_to_string(env)
}
