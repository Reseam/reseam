# Bundle layout

A bundle is a Gradle project.

```
my-bundle/
  manifest.toml
  settings.gradle.kts
  build.gradle.kts
  android-extension-module.gradle.kts
  gradlew
  gradle/
  apps/
  shared/
```

## `apps/<app>/`

One directory per target app.

```
apps/<app>/
  patch/
  extensions/
    <name>/
```

### `patch/`

A Kotlin subproject that compiles to a JAR. Depends on `app.reseam:reseam-patch-sdk`. Patch sources go in `src/main/kotlin/`.

### `extensions/<name>/`

Each extension compiles to a single DEX file, injected into the target app by a patch. Extension sources go in `src/main/java/` or `src/main/kotlin/`.

Extension build scripts set `extra["dexOutputName"]` and apply `android-extension-module.gradle.kts`.

## `shared/`

Extension code shared across apps. Same structure as an app extension. Extract when two or more extensions need the same code.

## What ends up in the signed bundle

`manifest.toml`, each `<app>-patches.jar`, and every `*.dex`. Nothing else (no sources, no Gradle scripts).

## Next

- [Writing patches](4_writing_patches.md)
- [Extensions](5_extensions.md)
