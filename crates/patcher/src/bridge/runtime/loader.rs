// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use jni::objects::{Global, JClass, JObject};
use jni::refs::IntoAuto;
use jni::{Env, jni_sig, jni_str};
use reseam_storage::ScratchDir;

use super::callback::load_class;
use super::class_loader::create_class_loader;
use super::discovery::read_declarations;
use super::jvm::{self, jvm_err};
use super::metadata::read_patch;
use crate::bridge::phase::LoadedBundle;
use crate::bundle::BundleInfo;
use crate::bundle::index::Declaration;
use crate::error::Result;
use crate::patch::Patch;

const PATCH_INTERFACE: &str = "app.reseam.patch.ReseamPatch";
const EXTERNAL_PATCH: &str = "app.reseam.patch.ExternalPatch";

/// A bundle's patch class loader; closed when its last patch drops.
pub(in crate::bridge) struct PatchLoader {
    reference: Global<JObject<'static>>,
    pub(in crate::bridge) bundle: LoadedBundle,
}

impl PatchLoader {
    pub(in crate::bridge) fn reference(&self) -> &JObject<'static> {
        self.reference.as_ref()
    }
}

impl Drop for PatchLoader {
    fn drop(&mut self) {
        #[cfg(not(target_os = "android"))]
        {
            let closed = (|| {
                jvm::get_or_init()?.attach_current_thread(|env| {
                    // A loader can drop while a failed JNI lookup is unwinding.
                    // Close with a clear environment, then restore the original exception.
                    let pending = env.exception_occurred().map(IntoAuto::auto);
                    env.exception_clear();
                    let result = jvm::with_frame(env, |env| {
                        env.call_method(
                            self.reference.as_ref(),
                            jni_str!("close"),
                            jni_sig!("()V"),
                            &[],
                        )?;
                        Ok(())
                    });
                    if let Some(pending) = pending {
                        match env.throw(&*pending) {
                            Err(jni::errors::Error::JavaException) | Ok(()) => {}
                            Err(error) => return Err(error.into()),
                        }
                    }
                    result
                })
            })();
            if let Err(error) = closed {
                tracing::warn!(%error, "failed to close patch class loader");
            }
        }
    }
}

/// Loads the patches `declarations` names, by declaring class. Only those
/// classes are initialized, so patches left out never run any code.
pub(crate) fn load_patches(
    jars: &[PathBuf],
    declarations: &BTreeMap<String, Vec<Declaration>>,
    directory: Arc<ScratchDir>,
    bundle: &BundleInfo,
) -> Result<Vec<Patch>> {
    if declarations.is_empty() {
        return Ok(Vec::new());
    }
    let vm = jvm::get_or_init()?;
    vm.attach_current_thread(|env| {
        jvm::with_frame(env, |env| {
            let loader = create_class_loader(env, jars)?;
            let retained_loader = Arc::new(PatchLoader {
                reference: env.new_global_ref(&loader)?,
                bundle: LoadedBundle::new(bundle, directory),
            });
            let loader = retained_loader.reference.as_ref();
            register_natives(env, loader)?;
            let patch_class = {
                let class = load_class(env, loader, PATCH_INTERFACE)?;
                JClass::cast_local(env, class)?
            };
            let external_class = {
                let class = load_class(env, loader, EXTERNAL_PATCH)?;
                JClass::cast_local(env, class)?
            };
            let mut found = Vec::new();
            for (class_name, declarations) in declarations {
                jvm::with_frame(env, |env| {
                    let class = {
                        let class = load_class(env, loader, class_name)?;
                        JClass::cast_local(env, class)?
                    };
                    read_declarations(
                        env,
                        &class,
                        declarations,
                        &patch_class,
                        &external_class,
                        &mut found,
                    )
                })?;
            }
            found
                .iter()
                .map(|patch| {
                    jvm::with_frame(env, |env| {
                        read_patch(
                            env,
                            patch,
                            &found,
                            &external_class,
                            &retained_loader,
                            &bundle.name,
                        )
                    })
                })
                .collect()
        })
    })
}

#[cfg(reseam_jni_bridge)]
fn register_natives(env: &mut Env<'_>, loader: &JObject<'_>) -> Result<()> {
    unsafe extern "C" {
        fn reseam_register_patch_natives(
            env: *mut jni::sys::JNIEnv,
            class: jni::sys::jclass,
        ) -> jni::sys::jint;
    }
    let native = load_class(env, loader, "app.reseam.patch.native.Native")?;
    // SAFETY: the references belong to this thread and frame. Registration
    // installs static generated entry points and retains no local references.
    let status = unsafe { reseam_register_patch_natives(env.get_raw(), native.as_raw()) };
    if status != jni::sys::JNI_OK {
        return Err(jvm_err(format!("register natives failed: {status}")));
    }
    Ok(())
}

#[cfg(not(reseam_jni_bridge))]
fn register_natives(_env: &mut Env<'_>, _loader: &JObject<'_>) -> Result<()> {
    Err(jvm_err(
        "patch loading is unavailable in a binding metadata build",
    ))
}
