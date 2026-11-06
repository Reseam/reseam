---
title: "API: Gradle plugin"
description: The plugins that build a bundle, their settings, and the properties the build reads.
---

# API: Gradle plugin

The Gradle plugin builds your bundle. The guide page is [Bundle projects](15_bundles.md).

## Plugins

You apply one plugin yourself, `app.reseam.workspace`, in `settings.gradle.kts`:

```kotlin
plugins {
    id("app.reseam.workspace") version "0.33.0"
}
```

Its version is the engine version your bundle is built for, and the patch API version it compiles against.

It applies the others for you, by folder:

| Plugin | Applied to | What it does |
|---|---|---|
| `app.reseam.workspace` | `settings.gradle.kts` | Adds a module for each folder of the layout, and the repositories they need. |
| `app.reseam.bundle` | the root project | Packs and signs the bundle. |
| `app.reseam.patches` | `apps/<app>/patch`, `shared/<name>/patch` | Compiles patches, and generates the bindings of the folder's extensions. |
| `app.reseam.extension` | `apps/<app>/extensions/<name>`, `shared/<name>` | Compiles an extension to DEX. A module with `src/main/js` is a JavaScript extension instead. |
| `app.reseam.settings` | `apps/<app>/settings`, `shared/<name>/settings` | Compiles settings and their strings, for patches and extensions both. |

A module outside the layout can apply `app.reseam.patches`, `app.reseam.extension`, or `app.reseam.settings` in its own `build.gradle.kts`, and connect to the others with the dependency configurations below.

## Extensions

### `reseam`

In a patch module's `build.gradle.kts`: the other bundles whose patches yours depend on. The build downloads each one and generates an [`ExternalPatch`](reference.md#externalpatch) for each of its patches, in the same package as the original.

| Member | Description |
|---|---|
| `bundle(index: String, version: String, signer: String? = null)` | A published bundle: the URL of its `patches.json`, the release to use, and optionally the signer's public key you expect. |
| `bundle(file: File)` | A bundle file you build next to yours: `bundle(file("../other/build/reseam/other.reseam"))`. |

```kotlin
reseam {
    bundle(index = "https://patches.example.com/patches.json", version = "1.4.0")
}
```

### `reseamStrings`

In a settings module with `src/main/strings`: the string table the build generates.

| Member | Description |
|---|---|
| `objectName: Property<String>` | The generated object's full name, such as `app.example.settings.ExampleStrings`. Required. |
| `tableId: Property<String>` | The table's name. Defaults to the object's simple name. |
| `assetDirectory: Property<String>` | Where the table is stored in the app. Defaults to `reseam/<bundle>/strings/<tableId>`. |

```kotlin
reseamStrings { objectName = "app.example.settings.ExampleStrings" }
```

## Dependency configurations

The layout connects modules for you. A module outside it uses these:

| Configuration | In | Adds |
|---|---|---|
| `reseamExtensions(project(":my-extension"))` | a patch module | a Kotlin extension, and generates its bindings |
| `reseamJavascript(project(":my-extension"))` | a patch module | a JavaScript extension, and generates its binding |
| `reseamSettings(project(":settings"))` | an extension | a settings module, so the extension can read its settings and strings |

A patch module uses a settings module with the usual `api(project(":settings"))`.

## Tasks

| Task | Description |
|---|---|
| `bundle` | Builds, packs, and signs the bundle into `build/reseam/<name>.reseam`. Packing loads every patch once to record its metadata, so a patch that fails to load fails the build. The build lists which module made each file, which sets every patch's [fingerprint](15_bundles.md#fingerprints). |
| `unsignedBundle` | Builds and packs the bundle without a key into `build/reseam/unsigned/<name>.reseam`, for [`reseam bundle sign`](/docs/cli/bundle/#reseam-bundle-sign) to sign on another machine. |
| `typecheck-<app>` | Type checks an app folder's JavaScript extensions, for a folder with a `package.json`. `bundle` runs it. |

## Properties

The build reads these from environment variables or Gradle properties.

| Environment | Gradle property | Description |
|---|---|---|
| `RESEAM_BIN` | `reseam.bin` | The `reseam` CLI. Otherwise, the one in the engine checkout `RESEAM_WORKSPACE` names, then `reseam` on your `PATH`. |
| `RESEAM_BUNDLE_KEY` | `reseam.signingKey` | The signing key `bundle` signs with. Defaults to `~/.reseam/bundle-signing.key`. |
| `RESEAM_RELEASE_VERSION` | `releaseTag` | The version recorded in the bundle. `releaseTag` may start with `v`, which is dropped. |
| `RESEAM_WORKSPACE` | `reseam.workspace` | A local engine checkout to build against instead of the published plugin. Only for working on the engine. |
| `ANDROID_HOME` or `ANDROID_SDK_ROOT` | | The Android SDK. Extensions compile against the newest `platforms/android-*/android.jar` in it. |
