// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON exports for the Kotlin SDK. Requests and responses are the types in
//! `dto`, serialized as is.

use boltffi::{data, export};
use serde::Serialize;

use crate::dto::{InspectRequest, PatchRequest};

#[export]
pub trait PatchEventSink {
    fn on_event(&self, event_json: String);
}

#[export]
pub fn inspect_json(request_json: String) -> Result<String, String> {
    let request: InspectRequest = serde_json::from_str(&request_json).map_err(display)?;
    json(&crate::inspect(&request).map_err(failure)?)
}

#[export]
pub fn patch_json(request_json: String, event_sink: impl PatchEventSink) -> Result<String, String> {
    let request: PatchRequest = serde_json::from_str(&request_json).map_err(display)?;
    let outcome = crate::patch(&request, |event| {
        event_sink.on_event(serde_json::to_string(&event).expect("event serializes"))
    })
    .map_err(failure)?;
    json(&outcome)
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    failure(anyhow::anyhow!("{error:#}"))
}

fn failure(error: anyhow::Error) -> String {
    serde_json::to_string(&crate::SdkError::from(&error)).unwrap_or_else(|_| format!("{error:#}"))
}

/// A constructor's error reaches the host through boltffi's last-error slot
/// in its `Debug` form, so that form is the JSON the host expects, not a
/// quoted string.
pub struct ConstructorFailure(String);

impl std::fmt::Debug for ConstructorFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
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

/// An opened input held for the host: the component paths, extracted from a
/// container when needed, stay valid until the host closes the inspection.
pub struct ApkInspection {
    opened: std::sync::Mutex<crate::inspect::OpenedApk>,
}

#[export]
impl ApkInspection {
    pub fn new(apk_path: String, split_paths: Vec<String>) -> Result<Self, ConstructorFailure> {
        let splits = split_paths
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>();
        let opened = crate::inspect::open_apk(
            std::path::Path::new(&apk_path),
            &splits,
            &reseam_apk::ApkFile::patch_options(),
        )
        .map_err(|error| ConstructorFailure(failure(error)))?;
        Ok(Self {
            opened: std::sync::Mutex::new(opened),
        })
    }

    pub fn metadata_json(&self) -> Result<String, String> {
        let mut opened = self.opened.lock().map_err(display)?;
        json(&crate::inspect::apk_metadata(&mut opened).map_err(failure)?)
    }

    pub fn base_path(&self) -> Result<String, String> {
        Ok(self
            .opened
            .lock()
            .map_err(display)?
            .apk
            .base()
            .path()
            .to_string_lossy()
            .into_owned())
    }

    pub fn split_paths(&self) -> Result<Vec<String>, String> {
        Ok(self.opened.lock().map_err(display)?.apk.components()[1..]
            .iter()
            .map(|apk| apk.path().to_string_lossy().into_owned())
            .collect())
    }

    pub fn application_icon(&self) -> Result<Option<ApplicationIcon>, String> {
        let mut opened = self.opened.lock().map_err(display)?;
        Ok(opened
            .apk
            .application_icon()
            .map_err(display)?
            .map(Into::into))
    }
}

#[data]
pub enum ApplicationIcon {
    Bitmap(Vec<u8>),
    Adaptive {
        background: IconLayer,
        foreground: IconLayer,
    },
}

#[data]
pub enum IconLayer {
    Bitmap(Vec<u8>),
    Color(u32),
}

impl From<reseam_apk::ApplicationIcon> for ApplicationIcon {
    fn from(icon: reseam_apk::ApplicationIcon) -> Self {
        match icon {
            reseam_apk::ApplicationIcon::Bitmap(bytes) => Self::Bitmap(bytes),
            reseam_apk::ApplicationIcon::Adaptive {
                background,
                foreground,
            } => Self::Adaptive {
                background: background.into(),
                foreground: foreground.into(),
            },
        }
    }
}

impl From<reseam_apk::IconLayer> for IconLayer {
    fn from(layer: reseam_apk::IconLayer) -> Self {
        match layer {
            reseam_apk::IconLayer::Bitmap(bytes) => Self::Bitmap(bytes),
            reseam_apk::IconLayer::Color(argb) => Self::Color(argb),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ApkInspection;

    #[test]
    fn a_failed_open_reports_the_problem_as_json_in_debug_form() {
        let path = std::env::temp_dir().join("reseam-not-an-apk.jpg");
        std::fs::write(&path, b"not a zip").unwrap();
        let error = ApkInspection::new(path.display().to_string(), Vec::new())
            .err()
            .expect("a jpeg is not an APK");
        let json: serde_json::Value = serde_json::from_str(&format!("{error:?}")).unwrap();
        assert_eq!(json["problem"]["type"], "unreadable_apk");
        assert_eq!(json["problem"]["path"], path.display().to_string());
    }
}
