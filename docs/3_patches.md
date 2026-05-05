# Patches

Patches are Kotlin. Sources live under `apps/<app>/patch/src/main/kotlin/`. Every patch is a top-level `val` built with the `patch(...)` DSL; the engine discovers them from the compiled JAR, not by file path.

## Shape of a patch

```kotlin
import app.reseam.patch.boolOption
import app.reseam.patch.compatibleWith
import app.reseam.patch.optionsOf
import app.reseam.patch.patch
import app.reseam.patch.stringOption

val myPatch = patch(
    name = "My patch",
    description = "Does something useful.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    enabledByDefault = true,
    options = optionsOf(
        stringOption("some_string", default = "default_value"),
        boolOption("some_bool", default = true),
    ),
) {
    execute { ctx ->
        ctx.log.info("Patch executed")
    }
}
```

## Metadata parameters

- `name`: human-readable label.
- `description`: one line.
- `compatibleWith`: packages and optional versions. Patches whose target package or version doesn't match the APK are skipped. `compatibleWith("com.example.app", "1.0", "1.1")` restricts to specific versions.
- `dependsOn`: other patches in the same bundle that must run first. Accepts `ReseamPatch` references or name strings.
- `enabledByDefault`: whether the patch is on unless the user disables it. Defaults to `true`.
- `options`: user-configurable parameters. See below.
- `settingsHost`: the patch (by `ReseamPatch` reference or name) that owns the on-device settings schema this patch contributes to. Optional.
- `settings`: a list of `SettingsSection` entries the host patch should expose as runtime toggles in Reseam Manager. Empty by default.

## Body helpers

Inside the trailing `patch { ... }` block, alongside `execute` and `afterDependents`, the builder also exposes:

- `dependsOn(...)`, `compatibleWith(...)`, `option(...)`, `settings(...)`, `settingsHost(...)`: append to the list-shaped parameters above.
- `extendWith("name.dex", ...)`: declare extension DEX files this patch injects. See [Extensions](4_extensions.md).

## Lifecycle

`execute { ctx -> }` runs once per target APK.

`afterDependents { ctx -> }` runs after every patch that depends on this one has finished. Most patches don't need it.

An uncaught exception marks the patch as failed; other patches keep running. When partial success is acceptable, prefer `ctx.log.warn(...)` and early return over throwing.

## Options

```kotlin
import app.reseam.patch.boolOption
import app.reseam.patch.optionsOf
import app.reseam.patch.pathOption
import app.reseam.patch.stringOption

val configurablePatch = patch(
    name = "Configurable patch",
    description = "Reads user-supplied values at patch time.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    options = optionsOf(
        stringOption("api_endpoint", default = "api.example.com"),
        boolOption("enable_logging", default = false),
        pathOption("assets_dir"),
    ),
) {
    execute { ctx ->
        val endpoint = ctx.options.string("api_endpoint") ?: "default.com"
        val logging = ctx.options.bool("enable_logging") ?: false

        ctx.options.path("assets_dir")?.let { dir ->
            val entries = dir.listContents()
            val data = dir.readFile("logo.png")
        }
    }
}
```

Declarations: `stringOption`, `boolOption`, `intOption`, `floatOption`, `stringListOption`, `pathOption`. Read values with `ctx.options.string(key)`, `.bool`, `.int`, `.float`, `.stringList`, `.path`. Each returns `null` when unset.

`pathOption` returns a `PathOptionValue` that exposes `listContents()` and `readFile(relativePath)` for reading from a user-selected directory.

## Dependencies

Declare a dependency when one patch needs another to have run first:

```kotlin
val runtimeHost = patch(/* ... */) { /* ... */ }

val disableAutoplay = patch(
    name = "Disable autoplay",
    description = "Stops videos from auto-playing.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    dependsOn = listOf(runtimeHost),
) {
    execute { ctx ->
        // runtimeHost has already applied
    }
}
```

Disabling a dependency skips its dependents too.

Next: [the runtime surface](3_1_runtime.md).
