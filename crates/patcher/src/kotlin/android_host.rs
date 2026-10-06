// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{OnceLock, RwLock};

use jni::objects::{Global, JObject};
use jni::{Env, JavaVM, jni_str};

use super::jvm::jvm_err;
use crate::error::Result;

struct AndroidHost {
    vm: JavaVM,
    loader: RwLock<Global<JObject<'static>>>,
}

static HOST: OnceLock<AndroidHost> = OnceLock::new();

fn host() -> Result<&'static AndroidHost> {
    HOST.get().ok_or_else(|| {
        jvm_err("Android host is not initialized; call ReseamAndroidHost.setClassLoader first")
    })
}

pub(super) fn java_vm() -> Result<&'static JavaVM> {
    Ok(&host()?.vm)
}

pub(super) fn configured_class_loader<'a>(env: &mut Env<'a>) -> Result<JObject<'a>> {
    let loader = host()?
        .loader
        .read()
        .map_err(|error| jvm_err(format!("Android class loader lock: {error}")))?;
    env.new_local_ref(loader.as_ref()).map_err(jvm_err)
}

/// Installs the parent loader patches delegate to. It must expose the patch API
/// and Kotlin runtime. A replacement must belong to the original host VM.
/// Null arguments, objects other than `ClassLoader`, JNI failures and poisoned
/// host state are errors.
pub fn install_class_loader(env: &mut Env<'_>, loader: JObject<'_>) -> Result<()> {
    super::jvm::with_frame(env, |env| {
        if loader.is_null() {
            return Err(jvm_err("Android classLoader must not be null"));
        }
        if !env.is_instance_of(&loader, jni_str!("java/lang/ClassLoader"))? {
            return Err(jvm_err("Android parent must be a ClassLoader"));
        }
        let vm = env.get_java_vm()?;
        let loader = env.new_global_ref(loader)?;
        let candidate = AndroidHost {
            vm,
            loader: RwLock::new(env.new_global_ref(&loader)?),
        };
        let installed = HOST.get_or_init(|| candidate);
        if installed.vm.get_raw() != env.get_java_vm()?.get_raw() {
            return Err(jvm_err("Android classLoader belongs to another VM"));
        }
        *installed
            .loader
            .write()
            .map_err(|error| jvm_err(format!("Android class loader lock: {error}")))? = loader;
        Ok(())
    })
}
