# Patches

Patches are Kotlin. Sources live under `apps/<app>/patch/src/main/kotlin/`. Every patch is a public top-level `val` built with `patch(...)`; the engine discovers them from the compiled jar, not by file path.

## Shape of a patch

```kotlin
import app.reseam.patch.Type
import app.reseam.patch.invoke
import app.reseam.patch.method
import app.reseam.patch.patch
import app.reseam.patch.settings.section
import app.reseam.patch.settings.skipWhen

val hideSponsoredAds = patch("Hide sponsored messages") {
    description("Removes promoted posts from channels.")
    compatibleWith("org.telegram.messenger"("12.7.1"))
    settings(telegramSettings, section("Ads", TelegramSettings.hideSponsoredAds))

    execute {
        addSponsoredMessages.skipWhen(TelegramSettings.hideSponsoredAds)
    }
}

val addSponsoredMessages = method("addSponsoredMessages") {
    strings("https://t\\.me/(\\w+)(?:/(\\d+))?")
    returns(Type.Void)
}
```

The patch comes first in the file. What it looks for is declared below it as [targets](4_targets.md); what it changes happens inside `execute`. Targets resolve when `execute` first uses them.

## Declaration

- `patch("Name") { }`: a patch users see and can toggle. The name is its identity: dependencies, selections, and logs use it.
- `patch { }`: an internal patch. Never listed by the CLI or Reseam Manager, never selected on its own; it runs when a patch depending on it runs. Use it for shared setup: a settings host, a runtime the app must start, a signature bypass.

Inside the block:

- `description("...")`: one line, or a multi-line string; indentation is trimmed.
- `compatibleWith(...)`: the apps the patch applies to. See below.
- `dependsOn(otherPatch, ...)`: patches that must run first. References, never names.
- `enabledByDefault(false)`: leave the patch off until the user turns it on.
- `hidden()`: keep a named patch off the lists.
- `stringOption(...)` and the other option constructors: declare an [option](#options).
- `settings(host, section(...), ...)`: register [settings](#settings) with a host, which becomes a dependency.
- `execute { }`: the patch body. The receiver is the [runtime](6_runtime.md).
- `afterDependents { }`: runs after every patch depending on this one has finished. Most patches do not need it.

## Compatibility

A patch names the packages it applies to, each with optional versions. A package name alone means every version:

```kotlin
compatibleWith("com.example.app", "com.example.app.lite")
```

`"package"("version", ...)` pins versions:

```kotlin
compatibleWith("org.telegram.messenger"("12.7.1", "12.7.2"))
```

One patch can cover several apps with different versions each. Call `compatibleWith` once per form, or define the package once and reuse it across the bundle:

```kotlin
val TELEGRAM = "org.telegram.messenger"("12.7.1")

compatibleWith(TELEGRAM)
```

The engine skips a patch whose package or version does not match the APK and reports why. A patch with no `compatibleWith` applies to every app.

## Dependencies

```kotlin
val antiDeleteRuntime = patch {
    compatibleWith(TELEGRAM)

    execute {
        appEntry.before { call(DeletedArchive.init, thisObject) }
    }
}

val antiDelete = patch("Recover deleted messages") {
    compatibleWith(TELEGRAM)
    dependsOn(antiDeleteRuntime)

    execute { }
}
```

A dependency runs before its dependents. Skipping a dependency skips its dependents too. An internal patch runs whenever an enabled patch depends on it.

## Lifecycle

`execute { }` runs once per target APK. `afterDependents { }` runs after every patch that depends on this one has finished.

An uncaught exception marks the patch as failed; the run continues, and patches depending on it are skipped. When partial success is acceptable, log a warning and return instead of throwing.

## Options

Options are values the user supplies when applying the patch. Declare them in the patch block and read them by reference in `execute`:

```kotlin
val cloneInstagram = patch("Clone Instagram") {
    compatibleWith(INSTAGRAM)
    val packageName = stringOption(
        "packageName",
        title = "Package name",
        description = "New package name for the cloned app",
        default = "com.instagram.android.clone",
    )

    execute {
        val newPackage = options[packageName]
    }
}
```

`stringOption`, `boolOption`, `intOption`, `floatOption`, `stringListOption`, `pathOption`. Each takes `key`, `title`, `description`, `default`, `required`; `stringOption` also takes `validValues`.

`options[option]` returns the value with the engine's defaults applied. It throws for an optional option without a default that the user left empty; `options.getOrNull(option)` returns null instead. A `pathOption` yields an `OptionPath` with `listContents()` and `readFile(relativePath)`.

From the CLI: `--option <patch>.<key>=<value>`.

## Settings

Settings live in the patched app and are shown by the settings screen a host installs. Declare them as properties of an object; the key derives from the object and property names unless given:

```kotlin
import app.reseam.patch.settings.toggle

object TelegramSettings {
    val hideSponsoredAds by toggle("Hide sponsored messages", default = true)
    val recoverDeleted by toggle(
        "Recover deleted messages",
        summary = "Keep messages others delete.",
        default = true,
    )
}
```

`toggle`, `text`, `folder`, `choice(title, default, choices = listOf(Choice(value, title)))`. Pass `key = "..."` to pin a key; renaming a property otherwise changes the key and resets what users chose.

A host is an internal patch that installs the settings runtime and screen for one app:

```kotlin
import app.reseam.patch.settings.settingsHost

val telegramSettings = settingsHost("telegram") {
    compatibleWith(TELEGRAM)

    install {
        appEntry.before { call(TelegramSettingsEntry.init, thisObject) }
        settingsFillItems.after { call(TelegramSettingsEntry.appendReseamItem, param(0), thisObject) }
        manifest.addActivity("app.reseam.telegram.settings.TelegramReseamSettingsActivity") {
            this["android:label"] = "Reseam Settings"
        }
    }
}
```

`install` runs after every patch that registered settings with the host, once the host has written `assets/reseam/settings.json` for the runtime to read. Toggles gate emitted code; see [Gates](5_code.md#gates).

Next: [Finding code in the app](4_targets.md).
