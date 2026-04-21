# Setup

## Prerequisites

- JDK 17.
- Android SDK with `ANDROID_HOME` set to its root. Extension modules invoke `d8` from `$ANDROID_HOME/build-tools/*/` and link against the latest `platforms/android-*/android.jar`.
- The `reseam` CLI, built from the Reseam repo or installed from a release.
- Git.

## First build

Generate a bundle signing key the first time:

```bash
reseam bundle keygen --out bundle-signing.key
```

Keep it private and store it outside the repository. The corresponding public key is embedded in `patches.json` so clients can verify the bundles you publish.

Build:

```bash
export RESEAM_BUNDLE_KEY=$PWD/bundle-signing.key
./gradlew bundle
```

Output: `build/bundle/<name>.reseam`.

Inspect and apply to a local APK to check:

```bash
reseam bundle list build/bundle/<name>.reseam
reseam patch target.apk \
  --bundle build/bundle/<name>.reseam \
  --output patched.apk
```

For the project layout itself, see [Bundles](2_bundles.md). For the full release flow, see [Publishing](5_publish.md).
