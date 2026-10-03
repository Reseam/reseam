---
description: The project layout, the files that configure it, and what the build produces.
---

# Bundle projects

A bundle is built from a Gradle project with a fixed layout. The `app.reseam.workspace` plugin sets up every module from the folders it finds, so modules don't need build scripts.

## Layout

```text
my-patches/
  manifest.toml
  settings.gradle.kts
  apps/<app>/patch/                  Kotlin patches for one app
  apps/<app>/extensions/<name>/      Java code added to that app
  shared/<name>/                     Java code used by several apps
  resources/                         files patches copy into apps (optional)
```

Each app's patches compile to one `<app>-patches.jar`. Each extension compiles to a DEX file.

## `manifest.toml`

```toml
[bundle]
name = "my-patches"
author = "Your Name"
description = "Patches for Example"
format_version = 1
```

- `name`: lowercase letters, digits, and hyphens. It names the output file and prefixes your patch references (`my-patches/<id>`).
- `author` and `description` are shown to users.
- `format_version` is `1`.

## `settings.gradle.kts`

```kotlin
pluginManagement {
    repositories {
        mavenCentral()
        gradlePluginPortal()
        maven("https://git.reseam.app/api/packages/reseam/maven") {
            mavenContent { includeGroupAndSubgroups("app.reseam") }
        }
    }
    (System.getenv("RESEAM_WORKSPACE") ?: providers.gradleProperty("reseam.workspace").orNull)
        ?.takeIf { it.isNotBlank() }
        ?.let { includeBuild(it) }
}

plugins {
    id("app.reseam.workspace") version "0.18.1"
}

rootProject.name = "my-patches"
```

The plugin version sets the patch SDK version and the engine your bundle is built for. Use a `reseam` CLI of the same version.

The `RESEAM_WORKSPACE` lines are only for working on the engine itself: they point the build at a local checkout of the engine instead of the published plugin.

## Building

```bash
./gradlew bundle
```

This compiles everything, packs it, and signs it into `build/reseam/<name>.reseam`. While packing, the build loads your patches once to record their names, options, and compatibility in the bundle, so code that fails at load time fails the build.

The build finds:

- **the CLI** at `RESEAM_BIN` (or `-Preseam.bin`), then in the engine checkout named by `RESEAM_WORKSPACE`, then on `PATH`;
- **the signing key** at `RESEAM_BUNDLE_KEY` (or `-Preseam.signingKey`), then `~/.reseam/bundle-signing.key`.

## Module build scripts

Add a `build.gradle.kts` to a module only to declare dependencies. Extension dependencies are covered in [Shipping your own code](11_extensions.md#dependencies); a patch module uses one to [depend on another bundle](4_patches.md#patches-from-another-bundle). Every module can use Maven Central, Google's Maven repository, and the Reseam registry.

## What's in a `.reseam` file

A ZIP archive with:

- `mimetype`: marks the file as a Reseam bundle;
- `manifest.toml`: the bundle's name, author, the engine version it was built with, every patch's metadata, and a SHA-256 hash of every other file;
- `manifest.pubkey` and `manifest.sig`: your public key and the signature over the manifest;
- the patch jars, extension DEX files, and `resources/`.

Changing any file breaks the signature or a hash, and the bundle won't load. Sources and build scripts are not included.

Next: [Publishing](14_publishing.md).
