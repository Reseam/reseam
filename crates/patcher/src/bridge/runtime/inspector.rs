// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compiles a snippet with the host's Kotlin compiler and runs it in a patch
//! runtime over a context, through the patch API's `Inspector`.

use std::path::Path;

use jni::objects::{JClass, JObject, JValue};
use jni::{Env, jni_sig, jni_str};
use reseam_journal::Savepoint;
use reseam_model::analysis::{Inspected, KotlinCompiler, Snippet};
use reseam_storage::ScratchDir;

use super::callback::{load_class, new_runtime};
use super::desktop::runtime_jar;
use super::jni_values::{string_array, string_of};
use super::jvm::{self, jvm_err};
use super::loader::register_natives;
use crate::bridge::inspection::{take_output, take_target};
use crate::bridge::phase::{ContextGuard, RunningBundle};
use crate::bridge::run_state::RunGuard;
use crate::bridge::savepoints::{open_savepoint, rollback_to};
use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::inspect::{compile_errors, source};

const INSPECTOR: &str = "app.reseam.patch.inspect.Inspector";

/// Runs `snippet` against the APK of `ctx`. Whatever it changes is undone
/// before this returns, so the APK is as it was.
pub fn inspect(
    ctx: &mut PatchContext<'_>,
    compiler: &KotlinCompiler,
    snippet: &Snippet,
) -> Result<Inspected> {
    let _run = RunGuard::enter()?;
    let output = ScratchDir::new("inspection")?;
    compile(compiler, snippet, output.path())?;
    let savepoint = Savepoint::FIRST;
    ctx.open_savepoint(savepoint);
    open_savepoint(savepoint)?;
    let inspected = execute(ctx, snippet, output.path());
    ctx.rollback_to(savepoint);
    rollback_to(savepoint)?;
    inspected
}

/// Compiles `snippet` into `output`, failing with the compiler's errors at snippet lines.
fn compile(compiler: &KotlinCompiler, snippet: &Snippet, output: &Path) -> Result<()> {
    let (text, offset) = source(snippet);
    let classpath =
        std::env::join_paths([runtime_jar()?, compiler.stdlib.clone().into()]).map_err(jvm_err)?;
    let classpath = unicode(Path::new(&classpath))?;
    let output = unicode(output)?;
    let messages = call_inspector(|env, _, inspector| {
        let jars = string_array(env, &compiler.jars)?;
        let classpath = env.new_string(classpath)?;
        let text = env.new_string(&text)?;
        let output = env.new_string(output)?;
        let call = env.call_static_method(
            inspector,
            jni_str!("compile"),
            jni_sig!("([Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
            &[
                JValue::Object(&jars),
                JValue::Object(&classpath),
                JValue::Object(&text),
                JValue::Object(&output),
            ],
        );
        let messages = jvm::outcome(env, call, "Inspector.compile")?.l()?;
        Ok(string_of(env, messages)?)
    })?;
    if messages.is_empty() {
        return Ok(());
    }
    let errors = compile_errors(&messages, offset);
    Err(if errors.is_empty() {
        jvm_err(messages)
    } else {
        PatcherError::Compile(errors)
    })
}

/// Runs the snippet compiled into `output` in a patch runtime over `ctx`.
fn execute(ctx: &mut PatchContext<'_>, snippet: &Snippet, output: &Path) -> Result<Inspected> {
    let method = match snippet {
        Snippet::Target(_) => jni_str!("inspectTarget"),
        Snippet::Code(_) => jni_str!("runSnippet"),
    };
    let path = unicode(output)?;
    let bundle = RunningBundle {
        name: "inspection".to_owned(),
        version: String::new(),
        license: String::new(),
        directory: output.to_path_buf(),
    };
    let guard = ContextGuard::enter(ctx, bundle)?;
    let outcome = call_inspector(|env, loader, inspector| {
        register_natives(env, loader)?;
        let runtime = new_runtime(env, loader)?;
        let path = env.new_string(path)?;
        let call = env.call_static_method(
            inspector,
            method,
            jni_sig!("(Lapp/reseam/patch/PatchRuntime;Ljava/lang/String;)V"),
            &[JValue::Object(&runtime), JValue::Object(&path)],
        );
        jvm::outcome(env, call, "Inspector").map(drop)
    });
    let native = guard.finish();
    let printed = take_output();
    if let Err(error) = outcome.and(native) {
        return Err(match printed {
            Some(output) => PatcherError::Snippet {
                output,
                reason: error.to_string(),
            },
            None => error,
        });
    }
    Ok(match snippet {
        Snippet::Target(_) => Inspected::Target(
            take_target().ok_or_else(|| jvm_err("the inspector reported no target"))?,
        ),
        Snippet::Code(_) => Inspected::Output(printed.unwrap_or_default()),
    })
}

/// Calls into the patch API's `Inspector`, loaded from the patch runtime on
/// the JVM's class path, with that class path's loader.
pub(super) fn call_inspector<T>(
    call: impl FnOnce(&mut Env<'_>, &JObject<'_>, &JClass<'_>) -> Result<T>,
) -> Result<T> {
    jvm::get_or_init()?.attach_current_thread(|env| {
        jvm::with_frame(env, |env| {
            let loader = env.call_static_method(
                jni_str!("java/lang/ClassLoader"),
                jni_str!("getSystemClassLoader"),
                jni_sig!("()Ljava/lang/ClassLoader;"),
                &[],
            );
            let loader = jvm::outcome(env, loader, "ClassLoader.getSystemClassLoader")?.l()?;
            let inspector = load_class(env, &loader, INSPECTOR)?;
            let inspector = JClass::cast_local(env, inspector)?;
            call(env, &loader, &inspector)
        })
    })
}

fn unicode(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| jvm_err(format!("path is not Unicode: {}", path.display())))
}
