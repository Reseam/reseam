# Setup

## Prerequisites

- **JDK 17.**
- **Android SDK.** Set `ANDROID_HOME` to the SDK root.
- **The `reseam` CLI.** Built from the Reseam repo or installed from a release.
- **Git.**

## Starting a new bundle

Clone the patch bundle template, detach from its history, and rename:

```bash
git clone <template-url> my-bundle
cd my-bundle
rm -rf .git
git init
```

- Edit `manifest.toml`: set `name`, `author`, and `description`.
- Edit `settings.gradle.kts`: change `rootProject.name`.
- Rename `apps/<example>/` to your target app's short name and update the `include(...)` lines in `settings.gradle.kts`.

## First build

```bash
reseam bundle keygen --out bundle-signing.key
export RESEAM_BUNDLE_KEY=$PWD/bundle-signing.key
./gradlew bundle
```

Verify:

```bash
reseam bundle list build/bundle/<bundle-name>.reseam
```

Apply to an APK during development:

```bash
reseam patch target.apk \
  --bundle build/bundle/<bundle-name>.reseam \
  --output patched.apk
```
