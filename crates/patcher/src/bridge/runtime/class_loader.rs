// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The class loader a bundle's patch jars load through: a `URLClassLoader` on
//! desktop JVMs, a `DexClassLoader` under the host app's loader on Android.

use std::path::PathBuf;

use jni::objects::{JObject, JValue};
#[cfg(not(target_os = "android"))]
use jni::refs::IntoAuto;
use jni::{Env, jni_sig, jni_str};

use super::jvm::jvm_err;
use crate::error::Result;

#[cfg(not(target_os = "android"))]
pub(in crate::bridge) fn create_class_loader<'a>(
    env: &mut Env<'a>,
    jars: &[PathBuf],
) -> Result<JObject<'a>> {
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
pub(in crate::bridge) fn create_class_loader<'a>(
    env: &mut Env<'a>,
    jars: &[PathBuf],
) -> Result<JObject<'a>> {
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
