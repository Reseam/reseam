---
description: Create a bundle from the template, build it, and run its patches on an app.
---

# Start a bundle

A bundle is the file you publish. It holds your patches, and any Kotlin code they add to apps. `reseam bundle init` starts one from the [patch bundle template](https://git.reseam.app/reseam/patches-template), which builds as it is and comes with two working patches.

## What you need

- **JDK 17.**
- **The Android SDK**, with `ANDROID_HOME` set to its folder. The build compiles your extension code against the newest `platforms/android-*/android.jar` it finds there.
- **The `reseam` CLI**, on your `PATH`. See [Install](/docs/cli/install/).

## 1. Create the bundle

```bash
reseam bundle init my-patches
cd my-patches
```

This downloads the template into the folder `my-patches` and names the bundle `my-patches`. Pick your own name: lowercase letters and digits, separated by single hyphens. Patch references use it, so keep it once you publish. The command also sets the Reseam Gradle plugin to the version of your CLI, so the bundle builds for the engine you test with.

The bundle contains:

```text
manifest.toml          the bundle's name, author, and description
settings.gradle.kts    the Reseam plugin version
apps/example/patch/    two example patches
apps/example/extensions/screenshots/
                       Kotlin code one of the patches adds to the app
.github/workflows/     a release workflow
```

Both example patches work on any app:

- **Enable debugging** marks the app debuggable in its manifest, so `adb` can attach to it.
- **Allow screenshots** sends the app's "block screenshots" calls to Kotlin code from the bundle, which removes that flag.

## 2. Make a signing key

Every bundle is signed. Users trust your public key once, and Reseam refuses any bundle that isn't signed with a key they trust. Make yours:

```bash
reseam bundle keygen --out ~/.reseam/bundle-signing.key
```

It prints the public key. You can print it again later with `reseam bundle pubkey --key ~/.reseam/bundle-signing.key`.

Keep the key file private, and out of the repository. The build reads it from this path. Back it up too: if you lose it, every user has to trust a new key.

## 3. Build

```bash
./gradlew bundle
```

The bundle is written to `build/reseam/my-patches.reseam`.

## 4. Try it on an app

List what is inside, then patch an APK you have:

```bash
reseam bundle list build/reseam/my-patches.reseam

reseam patch app.apk \
  --bundle build/reseam/my-patches.reseam \
  --trust <public key> \
  --enable "Enable debugging" \
  --output patched.apk
```

The example patches work on any app, so they start switched off, and `--enable` turns one on. Install the result with `adb install patched.apk`. If the app is already installed from a store, uninstall it first: Android only installs an update signed with the same key as the installed app.

## 5. Make it yours

1. Set `author` and `description` in `manifest.toml`.
2. Rename `apps/example` to the app you patch, and the `app.example` packages to your own.
3. Replace the example patches with yours.

Next: [Your first patch](2_first_patch.md).
