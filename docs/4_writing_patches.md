# Writing patches

Patches are Kotlin. Sources live under `apps/<app>/patch/src/main/kotlin/`. The engine discovers patches from the compiled JAR, not by file path.

## Shape of a patch

```kotlin
import app.reseam.patch.compatibleWith
import app.reseam.patch.patch

val removeLaunchTracking = patch(
    name = "Remove launch tracking",
    description = "Stops the app from reporting a launch event on startup.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    enabledByDefault = true,
) {
    execute { ctx ->
        // patch body
    }
}
```

Every patch is a top-level `val` built with the `patch(...)` DSL.

### Metadata

- **`name`** — human-readable label.
- **`description`** — one line, plain language.
- **`compatibleWith`** — packages and optional versions. Incompatible patches are skipped.
- **`dependsOn`** — other patches in the same bundle that must run first.
- **`enabledByDefault`** — whether the patch is on unless the user disables it. Default: false.
- **`options`** — user-configurable parameters.

### Lifecycle

`execute { ctx -> }` runs once per target APK. If the patch has dependents that need to finish first, use `afterDependents { ctx -> }`. Most patches don't need it.

## Options

```kotlin
import app.reseam.patch.optionsOf
import app.reseam.patch.stringOption

val rebrandApp = patch(
    name = "Rebrand app",
    description = "Replaces the app's display name.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    options = optionsOf(
        stringOption(
            "displayName",
            default = "My App",
            title = "Display name",
            description = "Name shown under the launcher icon",
        ),
    ),
) {
    execute { ctx ->
        val name = ctx.options.string("displayName")!!
    }
}
```

Options are typed: string, bool, int, float, string list, file path. Read them via `ctx.options.string(key)`, `ctx.options.bool(key)`, etc.

## The runtime surface

`ctx` (type `PatchRuntime`) exposes bytecode, manifest, resources, files, and logging.

### Bytecode

```kotlin
for (cls in ctx.bytecode.classes) { /* ... */ }
val target = ctx.bytecode.findClass("Lcom/example/app/SomeClass;")
    ?: error("class not found")
```

Methods expose `info`, `registersSize`, `insSize`, `returnType`, `parameterTypes`, and `instructions`.

Common mutations:

- `method.returnEarly(value)` — replace body with a constant return.
- `method.replaceAllStrings(old, new)` — rewrite `const-string` values.
- `method.insertInvokeStatic(index, className, methodName, proto, registers)` — splice a static call.
- `method.returnTrueWhen(setting)` — gate a boolean return on a setting value.

### Fingerprints

Find methods whose obfuscated names change between app versions:

```kotlin
import app.reseam.patch.fingerprint

private val shouldReportFingerprint = fingerprint {
    strings("app_launch_reported", "session_id")
    returnType("Z")
}
```

Check `fp.matched` before using `fp.method`. Warn on miss rather than crash.

### Manifest

```kotlin
ctx.manifest.document().use { doc ->
    doc.root["package"] = "com.example.app.clone"

    doc.findByTag("provider").forEach { provider ->
        val auth = provider["android:authorities"] ?: return@forEach
        provider["android:authorities"] = auth.replace("com.example.app", "com.example.app.clone")
    }

    val activity = doc.createElement("activity").apply {
        this["android:name"] = "com.example.app.addon.AddonActivity"
        this["android:exported"] = "true"
    }
    doc.findByTag("application").first().appendChild(activity)
}
```

Lookup: `doc.findByTag(tag)`, `doc.findByAttribute(key, value)`. Get/set: `el["android:name"]`. Resource refs: `el.setResourceRef("android:theme", resId)`.

### Resources

- `ctx.resources.poolAdd(string)` — add to the string pool, returns pool index.
- `ctx.resources.replaceEntry(resId, poolIdx)` — redirect a resource ID.

### Logging

`ctx.log.info(msg)`, `ctx.log.warn(msg)`, `ctx.log.error(msg)`. Collected per-patch, returned with the run result.

## Dependencies

Declare dependencies when one patch needs another to have run first:

```kotlin
val settingsRuntime = patch(/* ... */) { /* ... */ }

val disableAutoplay = patch(
    name = "Disable autoplay",
    description = "Stops videos from auto-playing.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    dependsOn = listOf(settingsRuntime),
) {
    execute { ctx ->
        // settingsRuntime has already applied
    }
}
```

Disabling a dependency skips its dependents too.

## Settings-integrated patches

For patches the user should toggle at runtime without repatching:

1. A settings-host patch registers a settings activity in the manifest and declares which DEX extensions to include.
2. Downstream patches declare `settingsHost`, list `SettingsSection` with `ToggleSetting` entries, and gate methods with `returnTrueWhen(setting)`.

```kotlin
import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.settings.ToggleSetting

val disableAutoplay = patch(
    name = "Disable autoplay",
    description = "Stops videos from auto-playing.",
    compatibleWith = listOf(compatibleWith("com.example.app")),
    settingsHost = hostPatch,
    dependsOn = listOf(hostPatch),
    settings = listOf(
        SettingsSection(
            title = "Playback",
            settings = listOf(ToggleSetting("disable_autoplay", default = true)),
        ),
    ),
) {
    execute { ctx ->
        autoplayFingerprint.method.returnTrueWhen(
            ToggleSetting("disable_autoplay", default = true),
        )
    }
}
```

## Error handling

An uncaught exception marks the patch as failed. Other patches keep running. Prefer `ctx.log.warn` and returning over throwing when partial success is acceptable.

## Next

- [Extensions](5_extensions.md)
- [Build and publish](6_build_and_publish.md)
