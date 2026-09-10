# Bundles

A bundle is what you publish: one signed `.reseam` file holding your compiled patches, any extra code they put into apps, and a manifest naming the bundle. Reseam Manager downloads it and applies the patches on the phone; the `reseam` CLI applies it on your machine.

You develop a bundle as a Gradle project with a fixed directory layout. The `app.reseam.workspace` plugin reads that layout and configures every module from it.

![The bundle project on the left: manifest.toml, settings.gradle.kts, gradlew, apps/telegram with a patch module and an anti-delete extension module (main and stub sources), and shared/settings-runtime. The gradlew bundle task in the middle compiles each patch module to a jar with classes.dex, runs d8 on each extension, then reseam bundle pack hashes every file and signs the manifest. The signed .reseam archive on the right holds mimetype, manifest.toml with a files table of SHA-256 hashes and the engine version, manifest.pubkey, manifest.sig, telegram-patches.jar, telegram-anti-delete.dex and settings-runtime.dex. No sources or Gradle scripts ship.](bundle-layout.svg)

## Project layout

```
my-bundle/
  manifest.toml
  settings.gradle.kts
  gradlew
  gradle/
  apps/<app>/
    patch/src/main/kotlin/...
    extensions/<name>/src/main/java/...
    extensions/<name>/src/stubs/java/...
  shared/<name>/src/main/java/...
```

- `apps/<app>/patch/`: the Kotlin patches for one app. Compiles to `<app>-patches.jar`, which carries both JVM classes and `classes.dex` so the engine loads it on the desktop JVM and on Android.
- `apps/<app>/extensions/<name>/`: Java compiled against `android.jar` into `<app>-<name>.dex`. See [Extensions](9_extensions.md).
- `shared/<name>/`: extensions used by more than one app. Compiles to `<name>.dex`.

Modules have no build script unless they declare dependencies:

```kotlin
dependencies {
    compileOnly(project(":shared:settings-runtime"))
}
```

## `settings.gradle.kts`

```kotlin
pluginManagement {
    repositories {
        mavenCentral()
        gradlePluginPortal()
        maven("https://git.reseam.app/api/packages/reseam/maven") {
            mavenContent { includeGroup("app.reseam") }
        }
    }
    (System.getenv("RESEAM_WORKSPACE") ?: providers.gradleProperty("reseam.workspace").orNull)
        ?.takeIf { it.isNotBlank() }
        ?.let { includeBuild(it) }
}

plugins {
    id("app.reseam.workspace") version "0.5.0"
}

rootProject.name = "my-bundle"
```

The plugin version is the SDK version. Every patch module gets the SDK dependency automatically.

## `manifest.toml`

```toml
[bundle]
name = "my-bundle"
author = "Your Name"
description = "One-line description"
format_version = 1
```

- `name`: short identifier, lowercase, no spaces. Names the output file and appears in `patches.json`.
- `author`, `description`: shown to users.
- `format_version`: currently `1`.
- `engine`: written by `reseam bundle pack`, never by hand. Bundles load on engines of the same major version, or the same minor while the major is 0.

Per-patch metadata lives in the patch code. Release metadata lives in `patches.json`, generated at publish time.

## What ships in the signed archive

`manifest.toml`, one `<app>-patches.jar` per app, and every extension `.dex`. No sources, no Gradle scripts. The engine links extension DEX files into the app when a patch first refers to a class they define, so patches never name them.

Next: [Your first patch](3_first_patch.md).
