// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `BoltFFI` surface of the application SDK. It lives one crate away from
//! the service because `BoltFFI` folds the exports of every direct dependency
//! into a binding root, and the patcher's exports belong to bundles, not apps.

#![expect(
    clippy::needless_pass_by_value,
    reason = "BoltFFI exports require owned values across the host boundary"
)]

mod device;
mod inspection;
mod selection;
mod signing;
mod storage;

/// Android apps install their class loader before loading bundles; it becomes
/// the parent of every bundle loader and supplies the shared patch runtime.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_reseam_sdk_ReseamAndroidHost_setClassLoader(
    mut env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    loader: jni::objects::JObject<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if let Err(error) = reseam_sdk::install_class_loader(env, loader) {
            env.throw_new(
                jni::jni_str!("java/lang/IllegalStateException"),
                jni::strings::JNIString::new(error.to_string()),
            )?;
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(test)]
mod tests {
    use crate::inspection::ApkInspection;
    use reseam_model::Problem;

    #[test]
    fn unreadable_inputs_report_typed_failures() {
        let tmp = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("absent.apk", None),
            ("plain.apk", Some(b"not a zip".as_slice())),
            ("truncated.apk", Some(b"PK\x03\x04".as_slice())),
        ] {
            let path = tmp.path().join(name);
            if let Some(bytes) = bytes {
                std::fs::write(&path, bytes).unwrap();
            }
            let error = ApkInspection::new(path.display().to_string(), Vec::new())
                .err()
                .unwrap();
            assert_eq!(
                error.problem,
                Problem::UnreadableApk {
                    path: path.display().to_string()
                }
            );
        }
    }
}
