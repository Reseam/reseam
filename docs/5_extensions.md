# Extensions

Extensions are Java or Kotlin code compiled to DEX and injected into the target app. Use them for anything the patched app needs to run at runtime: new Activities, settings UIs, download services, complex logic too large to splice inline.

## Layout

Each extension is a Gradle subproject under `apps/<app>/extensions/<name>/`.

```kotlin
// build.gradle.kts
extra["dexOutputName"] = "<app>-<name>.dex"
apply(from = rootDir.resolve("android-extension-module.gradle.kts"))

dependencies {
    add("compileOnly", project(":shared-<name>"))
}
```

- `dexOutputName`: the DEX filename. Keep it stable; patches reference it by name.
- `dexExcludeClasses`: optional comma-separated list of `.class` files to exclude from the DEX.
- Dependencies are `compileOnly`. Extensions are injected as separate DEX files at patch time.

## Shared modules

Code shared across apps lives under `shared/<name>/`. Same structure as an app extension. Extract when two or more extensions need the same code.

## Injection

A patch declares which DEX files to add to the APK. The DEX filenames must match the `dexOutputName` from the extension's build script. See the settings-host example in [writing patches](4_writing_patches.md).

## When to use an extension

- New Activities, Services, BroadcastReceivers.
- Code that needs Android framework APIs at runtime (UI, file I/O, preferences).
- Shared runtime infrastructure (settings storage, logging).

Patches *change* existing code. Extensions *add* new code.
