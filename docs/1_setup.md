# Setup

## Prerequisites

- JDK 17.
- Android SDK with `ANDROID_HOME` set to its root. The build runs `d8` from `$ANDROID_HOME/build-tools/*/` and compiles extensions against the latest `platforms/android-*/android.jar`.
- The `reseam` CLI from a release, the one matching the plugin version in `settings.gradle.kts`. The build packs and signs bundles with it. You only need to build it from source when you are changing the engine itself.
- Git.

Linux, macOS, and Windows all work. Commands on these pages use POSIX shell syntax for environment variables; in PowerShell set them first (`$env:RESEAM_BIN = "C:\\reseam\\reseam.exe"`) and run the command on its own line.

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

`RESEAM_WORKSPACE` is for working on the engine and a bundle at the same time: it points at a Reseam checkout, and the build then uses that checkout's SDK, Gradle plugin, and CLI instead of the published ones. With a released CLI on `PATH`, leave it unset.

Output: `build/reseam/<name>.reseam`.

Inspect the bundle and apply it to a local APK:

```bash
reseam bundle list build/reseam/<name>.reseam --trust <PUBLIC_KEY_HEX>
reseam patch target.apk \
  --bundle build/reseam/<name>.reseam \
  --trust <PUBLIC_KEY_HEX> \
  --output patched.apk
```

Next: [Bundles](2_bundles.md), then [Your first patch](3_first_patch.md). For the release flow, see [Publishing](11_publish.md).
