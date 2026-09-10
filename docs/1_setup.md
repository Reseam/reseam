# Setup

## Prerequisites

- JDK 17.
- Android SDK with `ANDROID_HOME` set to its root. The build runs `d8` from `$ANDROID_HOME/build-tools/*/` and compiles extensions against the latest `platforms/android-*/android.jar`.
- The `reseam` CLI, built from the Reseam repo or installed from a release. Use the release that matches the plugin version in `settings.gradle.kts`.
- Git.

## First build

Generate a bundle signing key once:

```bash
reseam bundle keygen --out ~/.reseam/bundle-signing.key
```

Keep it private and outside the repository. The public key is embedded in `patches.json` so clients can verify the bundles you publish.

Build:

```bash
./gradlew bundle
```

The build locates the CLI in this order: `RESEAM_BIN` (or `-Preseam.bin`), then `<RESEAM_WORKSPACE>/target/release/reseam` (or `-Preseam.workspace`), then `reseam` on `PATH`. The signing key comes from `RESEAM_BUNDLE_KEY`, `-Preseam.signingKey`, or `~/.reseam/bundle-signing.key`.

Set `RESEAM_WORKSPACE` to a checkout of the Reseam repo to build against its SDK and Gradle plugin from source instead of the published versions.

Output: `build/reseam/<name>.reseam`.

Inspect the bundle and apply it to a local APK:

```bash
reseam bundle list build/reseam/<name>.reseam --trust <PUBLIC_KEY_HEX>
reseam patch target.apk \
  --bundle build/reseam/<name>.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

Next: [Bundles](2_bundles.md). For the release flow, see [Publishing](10_publish.md).
