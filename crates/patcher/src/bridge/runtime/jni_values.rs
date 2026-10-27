// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reads Java values through their getters, with errors that name the getter.

use jni::objects::{JObject, JString, JValue, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::jvm::{self, jvm_err};
use crate::error::Result;

pub(in crate::bridge) fn object<'a>(
    env: &mut Env<'a>,
    target: &JObject<'_>,
    getter: &str,
    sig: &str,
) -> Result<JObject<'a>> {
    env.call_method(
        target,
        jni::strings::JNIString::new(getter),
        jni::signature::RuntimeMethodSignature::from_str(sig)?.method_signature(),
        &[],
    )
    .and_then(JValueOwned::l)
    .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

pub(in crate::bridge) fn optional_string(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<String>> {
    let value = object(env, target, getter, "()Ljava/lang/String;")?;
    if value.is_null() {
        return Ok(None);
    }
    string_of(env, value)
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter}: {e}")))
}

pub(in crate::bridge) fn string(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<String> {
    optional_string(env, target, getter)?
        .ok_or_else(|| jvm_err(format!("{getter}() returned null")))
}

pub(in crate::bridge) fn boolean(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<bool> {
    env.call_method(
        target,
        jni::strings::JNIString::new(getter),
        jni_sig!("()Z"),
        &[],
    )
    .and_then(JValueOwned::z)
    .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

pub(in crate::bridge) fn map_list<T>(
    env: &mut Env<'_>,
    list: &JObject<'_>,
    what: &str,
    mut read: impl FnMut(&mut Env<'_>, &JObject<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    if list.is_null() {
        return Err(jvm_err(format!("{what} returned null")));
    }
    let size = env
        .call_method(list, jni_str!("size"), jni_sig!("()I"), &[])
        .and_then(JValueOwned::i)
        .map_err(|error| jvm_err(format!("{what}.size(): {error}")))?;
    if size < 0 {
        return Err(jvm_err(format!("{what}.size() returned a negative size")));
    }
    (0..size)
        .map(|index| {
            jvm::with_frame(env, |env| {
                let item = env
                    .call_method(
                        list,
                        jni_str!("get"),
                        jni_sig!("(I)Ljava/lang/Object;"),
                        &[JValue::Int(index)],
                    )
                    .and_then(JValueOwned::l)
                    .map_err(|error| jvm_err(format!("{what}.get({index}): {error}")))?;
                if item.is_null() {
                    return Err(jvm_err(format!("{what}.get({index}) returned null")));
                }
                read(env, &item)
            })
        })
        .collect()
}

pub(in crate::bridge) fn list_strings(
    env: &mut Env<'_>,
    list: &JObject<'_>,
    what: &str,
) -> Result<Vec<String>> {
    map_list(env, list, what, |env, item| {
        let item = env.new_local_ref(item)?;
        string_of(env, item).map_err(|error| jvm_err(format!("{what} element: {error}")))
    })
}

pub(in crate::bridge) fn optional_strings(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<String>>> {
    let list = object(env, target, getter, "()Ljava/util/List;")?;
    if list.is_null() {
        return Ok(None);
    }
    list_strings(env, &list, getter).map(Some)
}

pub(in crate::bridge) fn strings(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Vec<String>> {
    optional_strings(env, target, getter).map(Option::unwrap_or_default)
}

pub(in crate::bridge) fn string_of(
    env: &mut Env<'_>,
    value: JObject<'_>,
) -> jni::errors::Result<String> {
    env.cast_local::<JString<'_>>(value)?.try_to_string(env)
}
