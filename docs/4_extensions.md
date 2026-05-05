# Extensions

![Diagram: an extension module containing Java and Kotlin source files is compiled through d8 from the Android SDK build-tools into a single DEX file named after its dexOutputName. A patch then calls extendWith("<name>.dex") inside its body to inject that DEX into the target APK alongside the existing classes.dex. Gradle produces the DEX at bundle time; the patch performs the injection at patch time.](extension-flow.svg)

Extensions are Java or Kotlin code compiled to one DEX file and injected into the target app by a patch. Use them when the patched app needs new code at runtime: Activities, Services, settings UIs, or logic too large to splice inline.

## Project layout

Each extension is a Gradle subproject under `apps/<app>/extensions/<name>/`, or `shared/<name>/` for code reused across apps. Its `build.gradle.kts`:

```kotlin
extra["dexOutputName"] = "<app>-<name>.dex"
apply(from = rootDir.resolve("android-extension-module.gradle.kts"))

dependencies {
    add("compileOnly", project(":shared-<name>"))
}
```

- `dexOutputName`: the filename of the produced DEX. Keep it stable; patches reference it by name.
- `dexExcludeClasses`: optional comma-separated list of `.class` files to exclude from the DEX (for build stubs that must not ship).
- Dependencies on shared modules are `compileOnly`. Each extension is emitted as its own DEX, so a non-`compileOnly` dependency would duplicate classes into multiple DEX files.

## Referencing from a patch

A patch declares which DEX files to inject from inside its body:

```kotlin
extendWith("example-addon.dex")
```

Multiple calls or multiple arguments are allowed. Filenames must match the `dexOutputName` from the extension modules.
