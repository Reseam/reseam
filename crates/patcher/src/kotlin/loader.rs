// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reseam_storage::ScratchDir;

use jni::objects::{Global, JClass, JObject, JObjectArray, JValue, JValueOwned};
use jni::refs::IntoAuto;
use jni::{Env, jni_sig, jni_str};

use super::jvm::{self, jvm_err};
use super::metadata::{object, read_patch, string};
use super::patch::load_class;
use crate::bundle::declarations::declarations;
use crate::bundle::index::{Declaration, MemberKind};
use crate::error::Result;
use crate::patch::Patch;

const PATCH_INTERFACE: &str = "app.reseam.patch.ReseamPatch";
const EXTERNAL_PATCH: &str = "app.reseam.patch.ExternalPatch";

pub(super) struct PatchLoader {
    reference: Global<JObject<'static>>,
    directory: Arc<ScratchDir>,
}

impl PatchLoader {
    pub(super) fn reference(&self) -> &JObject<'static> {
        self.reference.as_ref()
    }

    pub(super) fn directory(&self) -> &Path {
        self.directory.path()
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

pub(super) struct Found {
    pub(super) object: Global<JObject<'static>>,
    pub(super) declaration: String,
}

pub fn load_patches(
    jars: &[PathBuf],
    directory: Arc<ScratchDir>,
    bundle: &str,
) -> Result<Vec<Patch>> {
    let declarations = declarations(jars)?;
    if declarations.is_empty() {
        return Ok(Vec::new());
    }
    let vm = jvm::get_or_init()?;
    vm.attach_current_thread(|env| {
        jvm::with_frame(env, |env| {
            let loader = create_class_loader(env, jars)?;
            let retained_loader = Arc::new(PatchLoader {
                reference: env.new_global_ref(&loader)?,
                directory,
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
            for (class_name, declarations) in &declarations {
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
                            bundle,
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

fn retain_patch(
    env: &Env<'_>,
    found: &mut Vec<Found>,
    object: &JObject<'_>,
    id: &str,
) -> Result<()> {
    for seen in found.iter_mut() {
        if env
            .is_same_object(seen.object.as_ref(), object)
            .map_err(|error| jvm_err(format!("compare patch {id}: {error}")))?
        {
            if id < seen.declaration.as_str() {
                id.clone_into(&mut seen.declaration);
            }
            return Ok(());
        }
    }
    found.push(Found {
        object: env
            .new_global_ref(object)
            .map_err(|error| jvm_err(format!("retain patch {id}: {error}")))?,
        declaration: id.to_owned(),
    });
    Ok(())
}

// Reflection order determines the input order of independent patches. Only indexed
// members are invoked; reflection also checks accessibility and invocation arguments.
fn read_declarations(
    env: &mut Env<'_>,
    class: &JClass<'_>,
    declarations: &[Declaration],
    patch_class: &JClass<'_>,
    external_class: &JClass<'_>,
    found: &mut Vec<Found>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for kind in [MemberKind::Field, MemberKind::Method] {
        let (getter, signature) = match kind {
            MemberKind::Field => ("getFields", "()[Ljava/lang/reflect/Field;"),
            MemberKind::Method => ("getMethods", "()[Ljava/lang/reflect/Method;"),
        };
        let members = {
            let members = object(env, class, getter, signature)?;
            JObjectArray::<JObject<'_>>::cast_local(env, members)?
        };
        let count = members.len(env)?;
        for index in 0..count {
            jvm::with_frame(env, |env| {
                let member = members.get_element(env, index)?;
                if kind == MemberKind::Method
                    && env
                        .call_method(&member, jni_str!("getParameterCount"), jni_sig!("()I"), &[])
                        .and_then(JValueOwned::i)?
                        != 0
                {
                    return Ok(());
                }
                let owner = object(env, &member, "getDeclaringClass", "()Ljava/lang/Class;")?;
                let name = string(env, &member, "getName")?;
                let owner = string(env, &owner, "getName")?;
                let Some((index, declaration)) =
                    declarations.iter().enumerate().find(|(_, declaration)| {
                        declaration.kind == kind
                            && declaration.member == name
                            && declaration.owner == owner
                    })
                else {
                    return Ok(());
                };
                if !seen.insert(index) {
                    return Ok(());
                }
                let value = match kind {
                    MemberKind::Field => env.call_method(
                        &member,
                        jni_str!("get"),
                        jni_sig!("(Ljava/lang/Object;)Ljava/lang/Object;"),
                        &[JValue::Object(&JObject::null())],
                    ),
                    MemberKind::Method => env.call_method(
                        &member,
                        jni_str!("invoke"),
                        jni_sig!("(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;"),
                        &[
                            JValue::Object(&JObject::null()),
                            JValue::Object(&JObject::null()),
                        ],
                    ),
                }
                .and_then(JValueOwned::l)
                .map_err(|error| {
                    jvm_err(format!(
                        "patch declaration {} ({}.{}): {error}",
                        declaration.id, declaration.class_name, declaration.member
                    ))
                })?;
                if value.is_null() || !env.is_instance_of(&value, patch_class)? {
                    return Err(jvm_err(format!(
                        "patch declaration {} did not return a ReseamPatch",
                        declaration.id
                    )));
                }
                if !env.is_instance_of(&value, external_class)? {
                    retain_patch(env, found, &value, &declaration.id)?;
                }
                Ok(())
            })?;
        }
    }
    if seen.len() != declarations.len() {
        return Err(jvm_err(
            "indexed patch member is absent from its declaration class",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn create_class_loader<'a>(env: &mut Env<'a>, jars: &[PathBuf]) -> Result<JObject<'a>> {
    let url_class = env.find_class(jni_str!("java/net/URL"))?;
    let urls = env.new_object_array(
        i32::try_from(jars.len()).map_err(jvm_err)?,
        &url_class,
        JObject::null(),
    )?;
    for (i, jar) in jars.iter().enumerate() {
        let path = jar
            .to_str()
            .ok_or_else(|| jvm_err(format!("jar path is not Unicode: {}", jar.display())))?;
        let url: JObject<'_> = env
            .with_local_frame_returning_local::<_, JObject<'_>, jni::errors::Error>(8, |env| {
                let path = env.new_string(path)?;
                let file = env.new_object(
                    jni_str!("java/io/File"),
                    jni_sig!("(Ljava/lang/String;)V"),
                    &[JValue::Object(&path)],
                )?;
                let uri = env
                    .call_method(&file, jni_str!("toURI"), jni_sig!("()Ljava/net/URI;"), &[])?
                    .l()?;
                env.call_method(&uri, jni_str!("toURL"), jni_sig!("()Ljava/net/URL;"), &[])?
                    .l()
            })
            .map_err(|error: jni::errors::Error| {
                jvm_err(format!("jar URL for {}: {error}", jar.display()))
            })?;
        let url = url.auto();
        urls.set_element(env, i, &url)
            .map_err(|error| jvm_err(format!("set jar URL[{i}]: {error}")))?;
    }
    env.new_object(
        jni_str!("java/net/URLClassLoader"),
        jni_sig!("([Ljava/net/URL;)V"),
        &[JValue::Object(&urls)],
    )
    .map_err(|e| jvm_err(format!("URLClassLoader: {e}")))
}

#[cfg(target_os = "android")]
fn create_class_loader<'a>(env: &mut Env<'a>, jars: &[PathBuf]) -> Result<JObject<'a>> {
    for jar in jars {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(jar)?)?;
        archive.by_name("classes.dex").map_err(|error| {
            jvm_err(format!(
                "Android patch jar {} has no classes.dex: {error}",
                jar.display()
            ))
        })?;
    }
    let parent = super::android_host::configured_class_loader(env)?;
    let dex_paths = jars
        .iter()
        .map(|jar| {
            jar.to_str()
                .ok_or_else(|| jvm_err(format!("jar path is not Unicode: {}", jar.display())))
        })
        .collect::<Result<Vec<_>>>()?
        .join(":");
    let optimized_dir = jars
        .first()
        .and_then(|jar| jar.parent())
        .map(|dir| dir.join("dex-cache"))
        .ok_or_else(|| jvm_err("no patch jars available for Android DexClassLoader"))?;
    std::fs::create_dir_all(&optimized_dir).map_err(|e| {
        jvm_err(format!(
            "create DexClassLoader optimized directory {}: {e}",
            optimized_dir.display()
        ))
    })?;
    let dex_path = env
        .new_string(dex_paths)
        .map_err(|e| jvm_err(format!("DexClassLoader dexPath: {e}")))?;
    let optimized_path = env
        .new_string(
            optimized_dir
                .to_str()
                .ok_or_else(|| jvm_err("DexClassLoader optimized directory is not Unicode"))?,
        )
        .map_err(|e| jvm_err(format!("DexClassLoader optimizedDirectory: {e}")))?;
    env.new_object(
        jni_str!("dalvik/system/DexClassLoader"),
        jni_sig!(
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/ClassLoader;)V"
        ),
        &[
            JValue::Object(&dex_path),
            JValue::Object(&optimized_path),
            JValue::Object(&JObject::null()),
            JValue::Object(&parent),
        ],
    )
    .map_err(|e| jvm_err(format!("DexClassLoader: {e}")))
}
