---
description: The project layout, the files that configure it, and what the build produces.
---

# Bundle projects

A bundle is built from a Gradle project with a fixed layout. The `app.reseam.workspace` plugin sets up a module for every folder it finds, so most modules need no build script at all.

## Layout

```text
my-patches/
  manifest.toml
  settings.gradle.kts
  apps/<app>/patch/                  patches for one app
  apps/<app>/settings/               that app's settings and strings (optional)
  apps/<app>/extensions/<name>/      code added to that app
  shared/<name>/                     code added to several apps
  shared/<name>/patch/               patches several apps share (optional)
  shared/<name>/settings/            settings several apps share (optional)
  resources/                         files patches copy into apps (optional)
```

The build turns these into the files inside your bundle:

| Folder | Becomes |
|---|---|
| `apps/<app>/patch/` | `<app>-patches.jar` |
| `shared/<name>/patch/` | `<name>-patches.jar` |
| `apps/<app>/extensions/<name>/` | `<app>-<name>.dex` |
| `apps/<app>/settings/` | `<app>-settings.dex` |
| `resources/` | `resources/`, as it is |

When your extensions use the Kotlin standard library, the bundle also gets `kotlin.dex`, with only the parts they use.

## `manifest.toml`

```toml
[bundle]
name = "my-patches"
author = "Your Name"
description = "Patches for Example"
license = "GPL-3.0"
format_version = 1
```

- `name`: lowercase letters, digits, and hyphens. It names the output file, and starts your patch references (`my-patches/<id>`). Changing it after you publish breaks every reference to your patches.
- `author`, `description`, and `license` are shown to users. `license` is optional, and shows on your settings screen's about card.
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
    id("app.reseam.workspace") version "0.33.0"
}

rootProject.name = "my-patches"
```

The plugin version sets the patch API you compile against, and the engine your bundle is built for. Use a `reseam` CLI of the same version.

The `RESEAM_WORKSPACE` lines are only for working on the engine itself. They point the build at a local engine checkout instead of the published plugin.

## Building

```bash
./gradlew bundle
```

This compiles everything, packs it, and signs it into `build/reseam/<name>.reseam`. While packing, the build loads your patches once to record their names, options, and compatibility in the bundle, so code that fails when the bundle loads fails the build.

The build finds:

- **the CLI** at `RESEAM_BIN` (or `-Preseam.bin`), then in the engine checkout `RESEAM_WORKSPACE` names, then on your `PATH`;
- **the signing key** at `RESEAM_BUNDLE_KEY` (or `-Preseam.signingKey`), then `~/.reseam/bundle-signing.key`.

[API: Gradle plugin](reference_gradle.md) lists every property and task.

## Module build scripts

Add a `build.gradle.kts` to a module only to declare dependencies:

- An extension's dependencies are covered in [Shipping your own code](10_extensions.md#dependencies).
- A patch module uses one to [depend on another bundle](4_patches.md#patches-from-another-bundle).
- A settings module uses one to name its string table, with `reseamStrings`.

Every module can use Maven Central, Google's Maven repository, and the Reseam registry.

## Fingerprints

Packing gives every patch a fingerprint: a hash of the code it can run and the bundle files it uses. Apps that patch with your bundle compare fingerprints between releases, and update only the apps whose patches changed.

A patch's fingerprint covers:

- its own code and every function that code calls, in your patch jars and extensions,
- every extension class the patch names, as bindings do, whole,
- the other files in the jars its code comes from,
- the files in `resources/` whose path, folder, or name without extension the code holds as a string.

Write bundle paths as whole strings, like `files.copy("resources/logo.png", "assets/logo.png")`. A path built from parts while patching doesn't count, so changing that file doesn't update apps that are already patched.

## What's in a `.reseam` file

A ZIP archive with:

- `mimetype`: marks the file as a Reseam bundle.
- `manifest.toml`: your bundle's name, author, and description, the engine version it was built with, every patch's metadata and fingerprint, and a SHA-256 hash of every other file.
- `manifest.pubkey` and `manifest.sig`: your public key, and its signature over the manifest.
- The patch jars, the extension and settings DEX files, and `resources/`.

Changing any file breaks the signature or a hash, and the bundle won't load. Your sources and build scripts aren't included.

Next: [Publishing](16_publishing.md).
