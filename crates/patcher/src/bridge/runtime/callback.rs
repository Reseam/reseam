// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use jni::objects::{Global, JClass, JObject, JValue};
use jni::{Env, jni_sig, jni_str};

use super::jvm::{self, jvm_err};
use crate::bridge::phase::{ContextGuard, OutputGuard};
use crate::bridge::savepoints::SavepointAction;
use crate::context::PatchContext;
use crate::error::Result;
use crate::output::OutputContext;
use crate::patch::PatchPhase;
use reseam_journal::Savepoint;

pub(in crate::bridge) struct KotlinCallback {
    object: Global<JObject<'static>>,
    loader: Arc<super::loader::PatchLoader>,
}

impl KotlinCallback {
    pub(in crate::bridge) fn new(
        object: Global<JObject<'static>>,
        loader: Arc<super::loader::PatchLoader>,
    ) -> Self {
        Self { object, loader }
    }

    pub(in crate::bridge) fn invoke(&self, phase: PatchPhase<'_, '_>) -> Result<()> {
        match phase {
            PatchPhase::Execute(ctx) => self.invoke_with_context(ctx, "invokeExecute"),
            PatchPhase::Finalize(ctx) => self.invoke_with_context(ctx, "invokeAfterDependents"),
            PatchPhase::RewriteOutput(ctx) => self.invoke_with_output(ctx),
        }
    }

    fn invoke_with_context(&self, ctx: &mut PatchContext<'_>, method: &str) -> Result<()> {
        let guard = ContextGuard::enter(ctx, self.loader.bundle.running())?;
        let outcome = jvm::get_or_init()?.attach_current_thread(|env| {
            jvm::with_frame(env, |env| {
                call_with_runtime(env, self.object.as_ref(), self.loader.reference(), method)
            })
        });
        let native = guard.finish();
        outcome.and(native)
    }

    fn invoke_with_output(&self, ctx: &mut OutputContext<'_>) -> Result<()> {
        let guard = OutputGuard::enter(ctx, self.loader.bundle.running())?;
        let outcome = jvm::get_or_init()?.attach_current_thread(|env| {
            jvm::with_frame(env, |env| {
                let class = load_class(
                    env,
                    self.loader.reference(),
                    "app.reseam.patch.OutputRuntime",
                )?;
                let class = JClass::cast_local(env, class)?;
                let runtime = env
                    .new_object(class, jni_sig!("()V"), &[])
                    .map_err(|e| jvm_err(format!("construct OutputRuntime: {e}")))?;
                call_phase(
                    env,
                    &runtime,
                    "OutputRuntime",
                    "invokeAfterWrite",
                    self.object.as_ref(),
                )
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
    let run = crate::bridge::run_state::kotlin_run(|| {
        let class = load_class(env, loader, "app.reseam.patch.PatchRun")?;
        let class = JClass::cast_local(env, class)?;
        let object = env.new_object(class, jni_sig!("()V"), &[])?;
        for savepoint in crate::bridge::savepoints::open_savepoints() {
            call_savepoint(env, &object, SavepointAction::Open, savepoint)?;
        }
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
    call_phase(env, &runtime, "PatchRuntime", method, patch)
}

/// Calls `method` of `runtime`, an instance of `class`, with `patch`.
fn call_phase(
    env: &mut Env<'_>,
    runtime: &JObject<'_>,
    class: &str,
    method: &str,
    patch: &JObject<'_>,
) -> Result<()> {
    let call = env.call_method(
        runtime,
        jni::strings::JNIString::new(method),
        jni_sig!("(Lapp/reseam/patch/ReseamPatch;)V"),
        &[JValue::Object(patch)],
    );
    jvm::outcome(env, call, &format!("{method}({class})")).map(drop)
}

/// Tells the run's patch runtime about `savepoint`. Before a patch creates the
/// runtime there is nothing to tell; it opens the savepoints open then.
pub(in crate::bridge) fn savepoint(action: SavepointAction, savepoint: Savepoint) -> Result<()> {
    let Some(run) = crate::bridge::run_state::existing_kotlin_run() else {
        return Ok(());
    };
    jvm::get_or_init()?.attach_current_thread(|env| {
        jvm::with_frame(env, |env| {
            call_savepoint(env, run.as_ref().as_ref(), action, savepoint)
        })
    })
}

fn call_savepoint(
    env: &mut Env<'_>,
    run: &JObject<'_>,
    action: SavepointAction,
    savepoint: Savepoint,
) -> Result<()> {
    let method = match action {
        SavepointAction::Open => jni_str!("openSavepoint"),
        SavepointAction::Release => jni_str!("releaseSavepoint"),
        SavepointAction::Rollback => jni_str!("rollbackTo"),
    };
    let call = env.call_method(
        run,
        method,
        jni_sig!("(I)V"),
        &[JValue::Int(savepoint.id() as i32)],
    );
    jvm::outcome(env, call, "PatchRun savepoint").map(drop)
}

pub(in crate::bridge) fn load_class<'a>(
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
