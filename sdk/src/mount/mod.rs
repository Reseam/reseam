// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mounting a patched build over an installed app on a rooted device. The
//! build goes into a root manager module under `/data/adb/modules`, whose boot
//! script bind-mounts each patched file over the installed one. A bind mount
//! shares the device and inode of its source, even where the root manager
//! hides mounts, which is how the scripts tell a mounted file. Swapping code
//! under ART leaves its compiled code and profiles stale, so the app is
//! compiled again as an install would; like an install, a failed compile
//! leaves the app running from its APK, so its result is not checked.
//!
//! The SDK writes the scripts; a host runs them in a root shell, on the device
//! or over adb.

mod installed;

use std::collections::BTreeSet;
use std::fmt::Write;

use reseam_model::{MountNamespace, MountRequest, Problem, RootScript};

use crate::error::{HostError, Result};

pub use installed::installed_app;

const HELPERS: &str = include_str!("shell.sh");
const MOUNTED: &str = "mounted";

/// Prints what [`installed_app`] reads: the installed app's version and the
/// files in its folder. Prints nothing when the app is not installed.
pub fn query_script(package: &str) -> Result<RootScript> {
    let package = package_name(package)?;
    Ok(global(format!(
        r#"{HELPERS}
path=$(app_path {package})
[ -n "$path" ] || exit 0
echo "version_code=$(version_code {package})"
dumpsys package {package} | sed -n 's/^ *versionName=/version_name=/p' | head -n 1
for file in "${{path%/*}}"/*.apk "${{path%/*}}"/*.dm; do
    [ -f "$file" ] && echo "file=${{file##*/}}"
done
true
"#
    )))
}

/// Mounts the patched build over the installed app, and keeps it mounted
/// across reboots for as long as the app keeps the installed version. Fails
/// with `InstalledAppDiffers` when the build is for another version or other
/// splits than the installed app.
pub fn mount_script(request: &MountRequest) -> Result<RootScript> {
    let package = package_name(&request.package_name)?;
    let installed = &request.installed;
    let patched: BTreeSet<&str> = request
        .splits
        .iter()
        .map(|split| split.name.as_str())
        .collect();
    let present: BTreeSet<&str> = installed
        .splits
        .iter()
        .map(|split| split.name.as_str())
        .collect();
    if i64::from(request.version_code) != installed.version_code || patched != present {
        return Err(Problem::InstalledAppDiffers {
            package: package.to_owned(),
            installed: installed.version_name.clone(),
        }
        .into());
    }
    let mut files = vec![(installed.base_file.as_str(), request.base.as_str())];
    for split in &installed.splits {
        let patched = request
            .splits
            .iter()
            .find(|patched| patched.name == split.name)
            .expect("the patched splits are the installed ones");
        files.push((split.file.as_str(), patched.path.as_str()));
    }
    if let (Some(name), Some(path)) = (&installed.dex_metadata_file, &request.dex_metadata) {
        files.push((name.as_str(), path.as_str()));
    }
    let dir = module_dir(package);
    let mut script = format!(
        "set -e\n{HELPERS}\ndir={dir}\n[ -d \"$dir\" ] && unmount {package} \"$dir\"\nrm -rf \"$dir\"\nmkdir -p \"$dir/apk\"\n"
    );
    for (name, path) in files {
        writeln!(script, "cp {} \"$dir/apk/\"{}", quote(path), quote(name))
            .expect("writing to a String succeeds");
    }
    let label = request.label.replace(['\n', '\r'], " ");
    write!(
        script,
        r#"chown 1000:1000 "$dir/apk/"*
chmod 644 "$dir/apk/"*
chcon u:object_r:apk_data_file:s0 "$dir/apk/"*
cat > "$dir/module.prop" <<'RESEAM_EOF'
id={id}
name=Reseam: {label}
version={version}
versionCode={code}
author=Reseam
description=Mounts the patched {label} over the installed app.
RESEAM_EOF
cat > "$dir/service.sh" <<'RESEAM_EOF'
{boot}
RESEAM_EOF
chmod 755 "$dir/service.sh"
sh "$dir/service.sh"
mounted {package} "$dir" && recompile {package}
"#,
        id = module_id(package),
        version = installed.version_name.replace(['\n', '\r'], " "),
        code = installed.version_code,
        boot = boot_script(package, installed.version_code),
    )
    .expect("writing to a String succeeds");
    Ok(global(script))
}

/// Takes the patched build off the installed app and removes its module.
pub fn unmount_script(package: &str) -> Result<RootScript> {
    let package = package_name(package)?;
    let dir = module_dir(package);
    Ok(global(format!(
        "set -e\n{HELPERS}\ndir={dir}\nunmount {package} \"$dir\"\nrm -rf \"$dir\"\nrecompile {package}\nam force-stop {package}\n"
    )))
}

/// Prints whether the patched build covers the installed app, which
/// [`is_mounted`] reads.
pub fn status_script(package: &str) -> Result<RootScript> {
    let package = package_name(package)?;
    Ok(global(format!(
        "{HELPERS}\nmounted {package} {} && echo {MOUNTED} || echo unmounted\n",
        module_dir(package)
    )))
}

/// Whether the output of [`status_script`] says the build is mounted.
pub fn is_mounted(output: &str) -> bool {
    output.trim() == MOUNTED
}

/// Prints the installed app's own `file` from under the mount, so the app
/// keeps running patched meanwhile.
pub fn original_script(package: &str, file: &str) -> Result<RootScript> {
    let package = package_name(package)?;
    Ok(RootScript {
        script: format!(
            "set -e\n{HELPERS}\nmount -o rprivate none /\nunmount {package} {}\ncat \"$(app_dir {package})\"/{}\n",
            module_dir(package),
            quote(file)
        ),
        namespace: MountNamespace::Private,
    })
}

/// Installs the APKs and `.dm` files at `files` on the device as an update of
/// `package`, taking a mount off first, and prints what the package manager
/// answers: `Success`, or why it refused. The files keep their names.
pub fn install_script(package: &str, files: &[String]) -> Result<RootScript> {
    let package = package_name(package)?;
    let mut script = format!(
        "{HELPERS}\ndir={}\n[ -d \"$dir\" ] && unmount {package} \"$dir\"\nsession=$(pm install-create -r -d | sed -n 's/.*\\[\\([0-9]*\\)\\].*/\\1/p')\n",
        module_dir(package)
    );
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        writeln!(
            script,
            "pm install-write \"$session\" {} {} > /dev/null",
            quote(name),
            quote(path)
        )
        .expect("writing to a String succeeds");
    }
    script.push_str("pm install-commit \"$session\" 2>&1 || true\n");
    Ok(global(script))
}

fn boot_script(package: &str, version_code: i64) -> String {
    format!(
        r#"#!/system/bin/sh
{HELPERS}
until [ "$(getprop sys.boot_completed)" = 1 ]; do sleep 1; done
[ "$(version_code {package})" = {version_code} ] || exit 0
app=$(app_dir {package})
for file in "${{0%/*}}"/apk/*; do
    target="$app/${{file##*/}}"
    [ -f "$target" ] || continue
    same_file "$target" "$file" || mount -o bind "$file" "$target"
done
am force-stop {package}"#
    )
}

fn global(script: String) -> RootScript {
    RootScript {
        script,
        namespace: MountNamespace::Global,
    }
}

fn module_id(package: &str) -> String {
    format!("reseam-{package}")
}

fn module_dir(package: &str) -> String {
    format!("/data/adb/modules/{}", module_id(package))
}

/// `package` when it is a package name, which the scripts use unquoted.
fn package_name(package: &str) -> Result<&str> {
    let valid = !package.is_empty()
        && package.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    if valid {
        Ok(package)
    } else {
        Err(HostError::InvalidRequest("not a package name"))
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}
