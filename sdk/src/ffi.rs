// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON exports for the Kotlin SDK. Requests and responses are the types in
//! `dto`, serialized as is.

use boltffi::export;
use serde::Serialize;

use crate::dto::{InspectRequest, PatchRequest};

#[export]
pub trait PatchEventSink {
    fn on_event(&self, event_json: String);
}

#[export]
pub fn inspect_json(request_json: String) -> Result<String, String> {
    let request: InspectRequest = serde_json::from_str(&request_json).map_err(display)?;
    json(&crate::inspect(&request).map_err(display)?)
}

#[export]
pub fn patch_json(request_json: String, event_sink: impl PatchEventSink) -> Result<String, String> {
    let request: PatchRequest = serde_json::from_str(&request_json).map_err(display)?;
    let outcome = crate::patch(&request, |event| {
        event_sink.on_event(serde_json::to_string(&event).expect("event serializes"))
    })
    .map_err(display)?;
    json(&outcome)
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_reseam_sdk_ReseamAndroidHost_setClassLoader(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    loader: jni::objects::JObject<'_>,
) {
    if let Err(error) = reseam_patcher::kotlin::android_host::install_class_loader(&mut env, loader)
    {
        let _ = env.throw_new("java/lang/IllegalStateException", error);
    }
}
