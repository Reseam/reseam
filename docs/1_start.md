---
description: Copy the template, build it, and run its patches on an app.
---

# Start a bundle

A bundle is the file you publish: your patches, plus any code they add to apps. The fastest way to start one is the [patch bundle template](https://git.reseam.app/reseam/patches-template). It builds as it is and comes with two working patches.

## What you need

- JDK 17.
- The Android SDK, with `ANDROID_HOME` set to its folder. The build compiles your Java code against the newest `platforms/android-*/android.jar` it finds there.
- The `reseam` CLI, on your `PATH`. Use the version that matches the plugin version in the template's `settings.gradle.kts`. See [Install](/docs/cli/install/).
- Git.

## 1. Copy the template

```bash
git clone https://git.reseam.app/reseam/patches-template.git my-patches
cd my-patches
```

The template contains:

```text
manifest.toml          bundle name, author, description
settings.gradle.kts    Reseam plugin version
apps/example/patch/    two example patches in Kotlin
apps/example/extensions/screenshots/
                       Java code one of them adds to the app
.github/workflows/     a release workflow for GitHub
```

The example patches work on any app:

- **Enable debugging** marks the app debuggable in its manifest.
- **Allow screenshots** sends the app's "block screenshots" calls to Java code from the bundle, which strips that flag.

## 2. Make a signing key

Every bundle is signed. Users trust your public key once, and Reseam refuses bundles that don't match it.

```bash
reseam bundle keygen --out ~/.reseam/bundle-signing.key
```

It prints the public key. Keep the key file private and out of the repository; the build reads it from this path.

## 3. Build

```bash
./gradlew bundle
```

The bundle is written to `build/reseam/example-patches.reseam`.

## 4. Try it on an app

List what is inside, then patch an APK you have:

```bash
reseam bundle list build/reseam/example-patches.reseam

reseam patch app.apk \
  --bundle build/reseam/example-patches.reseam \
  --trust <public key> \
  --enable "Enable debugging" \
  --output patched.apk
```

`--enable` is needed because patches that work on any app are off until you pick them. Install the result with `adb install patched.apk`.

## 5. Make it yours

1. Set `name`, `author`, and `description` in `manifest.toml`, and `rootProject.name` in `settings.gradle.kts`.
2. Rename `apps/example` to the app you patch, and the `app.example` packages to your own.
3. Replace the example patches with yours.

Next: [Your first patch](2_first_patch.md).
