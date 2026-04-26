// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_FEATURE_KOTLIN").is_err() {
        write_empty_jni_natives()?;
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let jni_dir = manifest_dir.join("../../patch-api/generated/jni");
    let glue_path = jni_dir.join("jni_glue.c");

    if !glue_path.exists() {
        write_empty_jni_natives()?;
        return Ok(());
    }

    println!("cargo:rerun-if-changed={}", glue_path.display());

    if should_compile_jni_glue() {
        let jni_includes = jni_include_dirs()?;

        let mut build = cc::Build::new();
        build.file(&glue_path).include(&jni_dir);
        for include in jni_includes {
            build.include(include);
        }

        // The JNI glue is generated and intentionally keeps the standard JNI
        // parameter shape even when some exports do not use `env`/`cls`.
        build.flag_if_supported("-Wno-unused-parameter");

        build.compile("reseam_jni_glue");
    }

    let content = fs::read_to_string(&glue_path)?;
    let natives = parse_jni_exports(&content)?;

    let out = PathBuf::from(env::var("OUT_DIR")?);
    let mut code = String::new();

    code.push_str("extern \"C\" {\n");
    for n in &natives {
        code.push_str(&format!("    fn {}();\n", n.c_name));
    }
    code.push_str("}\n\n");

    code.push_str("fn jni_native_methods() -> Vec<jni::NativeMethod> {\n");
    code.push_str("    vec![\n");
    for n in &natives {
        code.push_str(&format!(
            "        jni::NativeMethod {{\
             name: jni::strings::JNIString::from(\"{}\"), \
             sig: jni::strings::JNIString::from(\"{}\"), \
             fn_ptr: {} as *mut std::ffi::c_void \
             }},\n",
            n.method_name, n.jni_sig, n.c_name
        ));
    }
    code.push_str("    ]\n}\n");

    fs::write(out.join("jni_natives.rs"), code)?;
    Ok(())
}

fn write_empty_jni_natives() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(
        out.join("jni_natives.rs"),
        "fn jni_native_methods() -> Vec<jni::NativeMethod> { Vec::new() }\n",
    )?;
    Ok(())
}

struct JniNative {
    c_name: String,
    method_name: String,
    jni_sig: String,
}

fn parse_jni_exports(content: &str) -> Result<Vec<JniNative>, String> {
    let mut natives = Vec::new();
    let prefix = "Java_app_reseam_patch_Native_";

    for line in content.lines() {
        if !line.starts_with("JNIEXPORT ") || !line.contains("JNICALL") {
            continue;
        }

        // JNIEXPORT <ret> JNICALL <c_name>(JNIEnv *env, jclass cls, <params>) {
        let ret_type = line
            .strip_prefix("JNIEXPORT ")
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("");

        let after_jnicall = match line.find("JNICALL ") {
            Some(i) => &line[i + 8..],
            None => continue,
        };
        let paren = match after_jnicall.find('(') {
            Some(i) => i,
            None => continue,
        };
        let c_name = after_jnicall[..paren].trim().to_string();

        if !c_name.starts_with(prefix) {
            continue;
        }

        let encoded = &c_name[prefix.len()..];
        let method_name = decode_jni_name(encoded);

        let params_str =
            &after_jnicall[paren + 1..after_jnicall.find(')').unwrap_or(after_jnicall.len())];
        let params: Vec<&str> = params_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .skip(2)
            .collect();

        let mut sig = String::from("(");
        for param in &params {
            sig.push_str(c_type_to_jni_sig(param)?);
        }
        sig.push(')');
        sig.push_str(ret_to_jni_sig(ret_type)?);

        natives.push(JniNative {
            c_name,
            method_name,
            jni_sig: sig,
        });
    }

    Ok(natives)
}

fn decode_jni_name(encoded: &str) -> String {
    encoded
        .replace("_1", "\x00")
        .replace('_', ".")
        .replace('\x00', "_")
}

fn c_type_to_jni_sig(param: &str) -> Result<&'static str, String> {
    let ty = param.split_whitespace().next().unwrap_or("");
    match ty {
        "jbyte" => Ok("B"),
        "jint" => Ok("I"),
        "jlong" => Ok("J"),
        "jshort" => Ok("S"),
        "jboolean" => Ok("Z"),
        "jstring" => Ok("Ljava/lang/String;"),
        "jbyteArray" => Ok("[B"),
        "jintArray" => Ok("[I"),
        "jshortArray" => Ok("[S"),
        "jobject" => Ok("Ljava/nio/ByteBuffer;"),
        other => Err(format!("unknown JNI param type: {other}")),
    }
}

fn ret_to_jni_sig(ret: &str) -> Result<&'static str, String> {
    match ret {
        "void" => Ok("V"),
        "jbyteArray" => Ok("[B"),
        "jint" => Ok("I"),
        "jlong" => Ok("J"),
        "jshort" => Ok("S"),
        "jboolean" => Ok("Z"),
        "jstring" => Ok("Ljava/lang/String;"),
        other => Err(format!("unknown JNI return type: {other}")),
    }
}

fn should_compile_jni_glue() -> bool {
    env::var_os("RESEAM_SKIP_JNI_GLUE").is_none()
}

fn jni_include_dirs() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        return android_jni_include_dirs();
    }

    let java_home = find_java_home()?;
    let jni_include = PathBuf::from(&java_home).join("include");
    let jni_platform = if target.contains("apple-darwin") {
        jni_include.join("darwin")
    } else if target.contains("windows") {
        jni_include.join("win32")
    } else {
        jni_include.join("linux")
    };
    Ok(vec![jni_include, jni_platform])
}

fn android_jni_include_dirs() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let ndk = resolve_android_ndk()
        .ok_or("Android NDK not found; set ANDROID_NDK_HOME or ANDROID_HOME/ANDROID_SDK_ROOT")?;
    let prebuilt = ndk.join("toolchains").join("llvm").join("prebuilt");
    let host = std::fs::read_dir(&prebuilt)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| path.join("sysroot").join("usr").join("include").is_dir())
        .ok_or_else(|| format!("Android NDK sysroot not found under {}", prebuilt.display()))?;
    Ok(vec![host.join("sysroot").join("usr").join("include")])
}

fn resolve_android_ndk() -> Option<PathBuf> {
    env::var_os("ANDROID_NDK_HOME")
        .map(PathBuf::from)
        .filter(|path| is_android_ndk(path))
        .or_else(|| {
            let sdk = env::var_os("ANDROID_HOME")
                .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
                .map(PathBuf::from)?;
            let ndk_bundle = sdk.join("ndk-bundle");
            if is_android_ndk(&ndk_bundle) {
                return Some(ndk_bundle);
            }
            let ndk_dir = sdk.join("ndk");
            let mut versions = std::fs::read_dir(ndk_dir)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| is_android_ndk(path))
                .collect::<Vec<_>>();
            versions.sort();
            versions.pop()
        })
}

fn is_android_ndk(path: &Path) -> bool {
    path.join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .is_dir()
}

fn find_java_home() -> Result<String, Box<dyn Error>> {
    if let Ok(home) = env::var("JAVA_HOME") {
        if Path::new(&home).is_dir() {
            return Ok(home);
        }
    }
    if let Ok(output) = std::process::Command::new("java")
        .arg("-XshowSettings:property")
        .arg("-version")
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("java.home") {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let path = val.trim();
                    if Path::new(path).is_dir() {
                        return Ok(path.to_string());
                    }
                }
            }
        }
    }
    Err("JAVA_HOME not set and java not found on PATH".into())
}
