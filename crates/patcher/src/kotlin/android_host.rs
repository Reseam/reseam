// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Mutex, OnceLock};

use jni::objects::{GlobalRef, JClass, JObject};
use jni::JavaVM;

static VM: OnceLock<JavaVM> = OnceLock::new();
static PATCH_CLASS_LOADER: OnceLock<Mutex<Option<PatchClassLoader>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct PatchClassLoader(GlobalRef);

// GlobalRef is safe to share across threads by JNI contract. The jni crate does
// not mark it Sync because it wraps a raw JNI reference.
unsafe impl Send for PatchClassLoader {}
unsafe impl Sync for PatchClassLoader {}

pub(super) fn java_vm() -> Result<&'static JavaVM, String> {
    VM.get().ok_or_else(|| {
        "Android JavaVM is not initialized; call AndroidPatchHost.setClassLoader first".to_string()
    })
}

pub(super) fn configured_class_loader<'a>(
    env: &mut jni::JNIEnv<'a>,
) -> Result<Option<JObject<'a>>, String> {
    let Some(loader) = current_patch_class_loader()? else {
        return Ok(None);
    };
    env.new_local_ref(loader.0.as_obj())
        .map(Some)
        .map_err(|error| format!("configured Android classLoader: {error}"))
}

fn current_patch_class_loader() -> Result<Option<PatchClassLoader>, String> {
    class_loader_slot()
        .lock()
        .map(|slot| slot.clone())
        .map_err(|_| "classLoader lock is poisoned".to_string())
}

fn set_patch_class_loader(env: &mut jni::JNIEnv<'_>, loader: JObject<'_>) -> Result<(), String> {
    if loader.is_null() {
        return Err("classLoader must not be null".to_string());
    }

    remember_java_vm(env)?;
    let global = env
        .new_global_ref(&loader)
        .map_err(|error| format!("failed to retain classLoader: {error}"))?;
    *class_loader_slot()
        .lock()
        .map_err(|_| "classLoader lock is poisoned".to_string())? = Some(PatchClassLoader(global));
    Ok(())
}

fn clear_patch_class_loader() -> Result<(), String> {
    *class_loader_slot()
        .lock()
        .map_err(|_| "classLoader lock is poisoned".to_string())? = None;
    Ok(())
}

fn class_loader_slot() -> &'static Mutex<Option<PatchClassLoader>> {
    PATCH_CLASS_LOADER.get_or_init(|| Mutex::new(None))
}

fn throw_host_error(env: &mut jni::JNIEnv<'_>, message: impl AsRef<str>) {
    let _ = env.throw_new("java/lang/IllegalStateException", message.as_ref());
}

fn remember_java_vm(env: &jni::JNIEnv<'_>) -> Result<(), String> {
    if VM.get().is_some() {
        return Ok(());
    }
    let vm = env
        .get_java_vm()
        .map_err(|error| format!("failed to get Android JavaVM: {error}"))?;
    let _ = VM.set(vm);
    Ok(())
}

#[no_mangle]
pub extern "system" fn Java_app_reseam_patch_AndroidPatchHost_setClassLoader(
    mut env: jni::JNIEnv<'_>,
    _class: JClass<'_>,
    loader: JObject<'_>,
) {
    if let Err(error) = set_patch_class_loader(&mut env, loader) {
        throw_host_error(&mut env, error);
    }
}

#[no_mangle]
pub extern "system" fn Java_app_reseam_patch_AndroidPatchHost_clearClassLoader(
    mut env: jni::JNIEnv<'_>,
    _class: JClass<'_>,
) {
    if let Err(error) = clear_patch_class_loader() {
        throw_host_error(&mut env, error);
    }
}
