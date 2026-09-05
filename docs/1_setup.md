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

Gradle needs to know which `reseam` CLI to use when packing the bundle. Pick one:

- `-Preseam.workspace=/path/to/reseam` (or `RESEAM_WORKSPACE`): point at a sibling Reseam checkout. Gradle uses `<workspace>/target/release/reseam` and includes the workspace's own Gradle build, so the patch SDK is compiled from source and any local SDK edits are picked up.
- `RESEAM_BIN=/abs/path/to/reseam`: point at a prebuilt CLI binary. The patch SDK is resolved from Maven (released versions only).

Without one of those the build fails with `RESEAM_BIN env var or -Preseam.workspace property required to locate the reseam CLI`.

Output: `build/bundle/<name>.reseam`.

Inspect and apply to a local APK to check:

```bash
reseam bundle list build/bundle/<name>.reseam --trust <PUBLIC_KEY_HEX>
reseam patch target.apk \
  --bundle build/bundle/<name>.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

## Tuning the patcher

The Kotlin patch host runs in an embedded JVM. Heap defaults to `256m`, enough for most APKs, but very large targets (Instagram, Facebook) can OOM during patch search-index construction. Bump it via `RESEAM_JVM_HEAP`:

```bash
RESEAM_JVM_HEAP=4g reseam patch target.apk --bundle ...
```

Don't set this preemptively; only when you actually see `OutOfMemoryError: Java heap space` from the patcher. Going past your machine's free RAM gets you a SIGKILL (exit 137), not a Java exception.

For the project layout itself, see [Bundles](2_bundles.md). For the full release flow, see [Publishing](5_publish.md).
