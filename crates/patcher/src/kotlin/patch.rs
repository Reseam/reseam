// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::JNIEnv;

use super::handles::ContextGuard;
use super::jvm::{self, jvm_err};
use crate::context::PatchContext;
use crate::error::Result;
use crate::patch::{Patch, PatchSpec};

/// A `ReseamPatch` object living in the JVM.
pub(super) struct KotlinPatch {
    pub spec: PatchSpec,
    pub object: GlobalRef,
    pub bundle_dir: PathBuf,
}

// SAFETY: a JNI global reference may be used from any thread; the jni crate
// leaves `GlobalRef` !Sync only because it wraps a raw pointer.
unsafe impl Send for KotlinPatch {}
unsafe impl Sync for KotlinPatch {}

impl Patch for KotlinPatch {
    fn spec(&self) -> &PatchSpec {
        &self.spec
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
        self.invoke(ctx, "execute")
    }

    fn after_dependents(&self, ctx: &mut PatchContext) -> Result<()> {
        self.invoke(ctx, "afterDependents")
    }
}

impl KotlinPatch {
    fn invoke(&self, ctx: &mut PatchContext, method: &str) -> Result<()> {
        let mut env = jvm::get_or_init()?
            .attach_current_thread()
            .map_err(|e| jvm_err(format!("attach thread: {e}")))?;
        let _guard = ContextGuard::enter(ctx, self.bundle_dir.clone());
        jvm::with_frame(&mut env, |env| {
            call_with_runtime(env, self.object.as_obj(), method)
        })
    }
}

/// Calls `patch.<method>(PatchRuntime)` with a fresh runtime built by the
/// patch's own class loader.
fn call_with_runtime(env: &mut JNIEnv<'_>, patch: &JObject<'_>, method: &str) -> Result<()> {
    let class = env
        .call_method(patch, "getClass", "()Ljava/lang/Class;", &[])
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("patch.getClass(): {e}")))?;
    let loader = env
        .call_method(&class, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("patch class loader: {e}")))?;
    let runtime_class = load_class(env, &loader, "app.reseam.patch.PatchRuntime")?;
    let runtime = env
        .new_object(JClass::from(runtime_class), "()V", &[])
        .map_err(|e| jvm_err(format!("construct PatchRuntime: {e}")))?;
    let call = env.call_method(
        patch,
        method,
        "(Lapp/reseam/patch/PatchRuntime;)V",
        &[JValue::Object(&runtime)],
    );
    match (call, jvm::take_pending_exception(env)) {
        (_, Some(exception)) => Err(jvm_err(format!("{method}(PatchRuntime): {exception}"))),
        (Err(e), None) => Err(jvm_err(format!("{method}(PatchRuntime): {e}"))),
        (Ok(_), None) => Ok(()),
    }
}

pub(super) fn load_class<'a>(
    env: &mut JNIEnv<'a>,
    loader: &JObject<'_>,
    name: &str,
) -> Result<JObject<'a>> {
    let name_j = env
        .new_string(name)
        .map_err(|e| jvm_err(format!("new_string: {e}")))?;
    env.call_method(
        loader,
        "loadClass",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        &[JValue::Object(&name_j)],
    )
    .and_then(|v| v.l())
    .map_err(|e| jvm_err(format!("loadClass({name}): {e}")))
}
