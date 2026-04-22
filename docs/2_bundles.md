# Bundles

A bundle is a Gradle project that builds into a signed `.reseam` archive.

![Diagram: a bundle project directory (manifest.toml, gradle files, apps/<app>/patch, apps/<app>/extensions, shared/) on the left. The reseam bundle pack command hashes every payload file with SHA-256 and signs the manifest with Ed25519, producing the .reseam archive on the right containing mimetype, a rewritten manifest.toml with a [files] table, manifest.pubkey, manifest.sig, and the compiled patch JAR plus extension DEX files. Sources and build scripts stay out of the archive.](bundle-layout.svg)

## Project layout

```
my-bundle/
  manifest.toml
  settings.gradle.kts
  build.gradle.kts
  android-extension-module.gradle.kts
  gradlew
  gradle/
  apps/<app>/
    patch/
    extensions/<name>/
  shared/<name>/
```

- `apps/<app>/patch/`: a Kotlin subproject that compiles to a JAR. Depends on `app.reseam:reseam-patch-sdk`. Patch sources live under `src/main/kotlin/`. The JAR is renamed to `<app>-patches.jar` when staged into the bundle.
- `apps/<app>/extensions/<name>/`: a Java or Kotlin subproject that compiles to one DEX file. See [Extensions](4_extensions.md).
- `shared/<name>/`: extension code reused across apps. Same shape as an app extension.

## `manifest.toml`

Sits at the bundle root. All four fields are required.

```toml
[bundle]
name = "my-bundle"
author = "Your Name"
description = "One-line description"
format_version = 1
```

- `name`: short identifier, lowercase, no spaces. Shown by `reseam bundle list` and embedded in `patches.json`.
- `author`: the publisher's name or handle.
- `description`: one line.
- `format_version`: currently `1`. The engine refuses to load bundles whose `format_version` it doesn't recognize.

Per-patch metadata (name, description, compatibility, options) lives in the patch code. Release metadata (version, download URL) lives in `patches.json`, generated at publish time.

## What ships in the signed archive

Only `manifest.toml`, one `<app>-patches.jar` per app, and every `*.dex` produced by the extension modules. No sources, no Gradle scripts.
