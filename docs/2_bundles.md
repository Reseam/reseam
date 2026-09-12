# Bundles

A bundle is the file you publish and users install: every patch you maintain, for as many apps as you like, plus the [extension code](9_extensions.md) those patches put into apps and a manifest naming the bundle. It is signed with your key; Reseam Manager and the CLI refuse a bundle whose signature does not match a key the user trusts. It has its own version, independent of the engine and of the apps it patches.

You develop it as a Gradle project with a fixed layout. The `app.reseam.workspace` plugin configures every module from the directories, so there is one build command and no build scripts to maintain.

![The bundle project on the left: manifest.toml, settings.gradle.kts, gradlew, apps/example with a patch module and an ads extension module (main and stub sources), and shared/settings-runtime. The gradlew bundle task in the middle compiles each patch module to a jar with classes.dex, runs d8 on each extension, then reseam bundle pack hashes every file and signs the manifest. The signed .reseam archive on the right holds mimetype, manifest.toml with a files table of SHA-256 hashes and the engine version, manifest.pubkey, manifest.sig, example-patches.jar, example-ads.dex and settings-runtime.dex. No sources or Gradle scripts ship.](bundle-layout.svg)

## Project layout

```text
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
- `apps/<app>/extensions/<name>/`: Java compiled against `android.jar` into `<app>-<name>.dex`. See [Shipping your own code](9_extensions.md).
- `shared/<name>/`: extensions used by more than one app. Compiles to `<name>.dex`.

Modules have no build script unless they declare dependencies:

```kotlin
dependencies {
    compileOnly(project(":shared:settings-runtime"))
    implementation("org.lsposed.hiddenapibypass:hiddenapibypass:6.1")
}
```

Maven Central, Google's Maven, and the Reseam registry are available to every module. See [Shipping your own code](9_extensions.md) for what each dependency scope means in an extension, and [Depending on another bundle](4_patches.md#depending-on-another-bundle) for a patch module's `reseam { bundle(...) }` block.

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
    id("app.reseam.workspace") version "0.5.0"
}

rootProject.name = "my-bundle"
```

The `RESEAM_WORKSPACE` lines make the build use an engine checkout instead of the published plugin and SDK. They only matter when you change the engine alongside your patches; a bundle built with the released CLI does not need them.

The plugin version is the SDK version; every patch module gets the SDK dependency from it.

## `manifest.toml`

```toml
[bundle]
name = "my-bundle"
author = "Your Name"
description = "One-line description"
format_version = 1
```

- `name`: lowercase letters, digits, and hyphens. Names the output file, appears in `patches.json`, and qualifies patch references (`<name>/<id>`) from other bundles.
- `author`, `description`: shown to users.
- `format_version`: currently `1`.
- `engine`: written by `reseam bundle pack`, never by hand. Bundles load on engines of the same major version, or the same minor while the major is 0.

Next: [Your first patch](3_first_patch.md).
