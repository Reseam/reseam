// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use jni::objects::{Global, JClass, JObject, JValue};
use jni::{Env, jni_sig, jni_str};

use super::handles::ContextGuard;
use super::jvm::{self, jvm_err};
use crate::context::PatchContext;
use crate::error::Result;
use crate::patch::PatchPhase;

pub(super) struct KotlinCallback {
    object: Global<JObject<'static>>,
    loader: Arc<super::loader::PatchLoader>,
}

impl KotlinCallback {
    pub(super) fn new(
        object: Global<JObject<'static>>,
        loader: Arc<super::loader::PatchLoader>,
    ) -> Self {
        Self { object, loader }
    }

    pub(super) fn invoke(&self, phase: PatchPhase, ctx: &mut PatchContext<'_>) -> Result<()> {
        let method = match phase {
            PatchPhase::Execute => "invokeExecute",
            PatchPhase::Finalize => "invokeAfterDependents",
        };
        let guard = ContextGuard::enter(ctx, self.loader.directory().to_path_buf())?;
        let outcome = jvm::get_or_init()?.attach_current_thread(|env| {
            jvm::with_frame(env, |env| {
                call_with_runtime(env, self.object.as_ref(), self.loader.reference(), method)
            })
        });
        let native = guard.finish();
        outcome.and(native)
    }
}

fn call_with_runtime(
    env: &mut Env<'_>,
    patch: &JObject<'_>,
    loader: &JObject<'_>,
    method: &str,
) -> Result<()> {
    let run = super::handles::kotlin_run(|| {
        let class = load_class(env, loader, "app.reseam.patch.PatchRun")?;
        let class = JClass::cast_local(env, class)?;
        let object = env.new_object(class, jni_sig!("()V"), &[])?;
        Ok(env.new_global_ref(object)?)
    })?;
    let runtime_class = load_class(env, loader, "app.reseam.patch.PatchRuntime")?;
    let runtime_class = JClass::cast_local(env, runtime_class)?;
    let runtime = env
        .new_object(
            runtime_class,
            jni_sig!("(Lapp/reseam/patch/PatchRun;)V"),
            &[JValue::Object(run.as_ref().as_ref())],
        )
        .map_err(|e| jvm_err(format!("construct PatchRuntime: {e}")))?;
    let call = env.call_method(
        &runtime,
        jni::strings::JNIString::new(method),
        jni_sig!("(Lapp/reseam/patch/ReseamPatch;)V"),
        &[JValue::Object(patch)],
    );
    match (call, jvm::take_pending_exception(env)?) {
        (_, Some(exception)) => Err(jvm_err(format!("{method}(PatchRuntime): {exception}"))),
        (Err(e), None) => Err(jvm_err(format!("{method}(PatchRuntime): {e}"))),
        (Ok(_), None) => Ok(()),
    }
}

pub(super) fn load_class<'a>(
    env: &mut Env<'a>,
    loader: &JObject<'_>,
    name: &str,
) -> Result<JObject<'a>> {
    let name_j = env
        .new_string(name)
        .map_err(|e| jvm_err(format!("new_string: {e}")))?;
    env.call_method(
        loader,
        jni_str!("loadClass"),
        jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
        &[JValue::Object(&name_j)],
    )
    .and_then(jni::JValueOwned::l)
    .map_err(|e| jvm_err(format!("loadClass({name}): {e}")))
}
