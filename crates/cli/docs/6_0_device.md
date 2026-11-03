---
title: Device
description: Install a patched app over adb, compiled at install, or mount it over the app on a rooted phone.
---

# `reseam install`, `mount`, and `unmount`

These commands put a patched app on a phone connected over adb. They need `adb` from Android platform-tools on your `PATH`, and USB or wireless debugging turned on.

```bash
reseam install app-patched.apks
```

## `reseam install`

Installs an APK, APKM, XAPK, or APKS, or a base APK with `--split` for each split, as an update of the installed app or a new one. Each APK goes in with its dex metadata, so Android compiles the app with its baseline profile while it installs. Without it, the app runs uncompiled, and slower, until the phone's idle maintenance compiles it.

Android only installs an update signed with the same key as the installed app. Sign every version you install with the same key; see [signing](2_0_patch.md#signing).

## `reseam mount`

```bash
reseam patch app.apk --bundle patches.reseam --trust <key> --mount --output app-mounted.apks
reseam mount app-mounted.apks
```

Mounts a build made with `reseam patch --mount` over the app already installed on a rooted phone. The app keeps its signature, its data, and its store updates. The build goes into a root manager module, which mounts it again after every reboot for as long as the installed app stays at the version the build was made from.

The phone needs a root manager that grants `su` to the adb shell; it usually asks once. The build has to match the installed app: the same version code and the same splits. Otherwise the command fails and changes nothing.

Android installs only the splits a phone can use, so an app installed from an APKM often has fewer splits than the APKM. Patch the APKs the phone has: `adb shell pm path <package>` lists them, and `adb pull` copies them.

## `reseam unmount`

```bash
reseam unmount com.example.app
```

Takes the mounted build off the installed app and removes its module. The app runs as installed again.

## Things to know

- With several devices connected, pick one with `--device <serial>`, as `adb devices` lists it, or set `ANDROID_SERIAL`.
- `mount` copies the build through `/data/local/tmp/reseam-mount` on the phone and deletes it afterwards.
- After a store update of a mounted app, patch the new version with `--mount` and mount it again.
