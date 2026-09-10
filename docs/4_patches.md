# Patches

A patch is one change to one app that a user can switch on: hide sponsored posts, unlock a paid feature, stop an update prompt. Reseam applies it on the phone by rewriting the app itself: it finds the methods the change concerns, alters their code, and re-signs the APK. The patched app installs next to the original.

To you, a patch is a Kotlin value with two halves. The first is what the engine and the user need to know before anything runs: a name and description, which apps and versions it applies to, other patches that must run first, and values the user may set. The second is `execute`, the code that makes the change.

You write patches in files under `apps/<app>/patch/src/main/kotlin/`. Each patch is a public top-level `val`. The engine finds patches by looking through the compiled classes for such values, so file names and property names are yours to choose; only the name inside `patch("...")` is what users see.

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

The patch comes first in the file. What it looks for is declared below it as [targets](5_targets.md); what it changes happens inside `execute`. Targets resolve when `execute` first uses them.

## How a patch runs

Three moments matter, and the API is only partly available in each.

1. **Load.** The engine opens the bundle and initialises the patch jar's top-level values: every `patch { }` and every target. It reads metadata (name, description, compatibility, dependencies, options) off the public top-level `val`s of type `ReseamPatch`. Nothing has looked at the app yet. A target is only a description here; `.method`, `.explain()`, `options[...]` and every scope throw `This API is only available while a patch is executing`.
2. **Selection.** The user, or the CLI flags, decide which patches run. A patch is skipped when its package or version does not match the APK, when a dependency was skipped, or when it is disabled.
3. **Patch time.** `execute { }` runs once with the runtime as receiver. A target resolves the first time this patch touches it and stays cached until the patch finishes; the next patch resolves it again against the app as that patch sees it. Code blocks such as `before { }` run immediately and emit instructions into the method. After every patch that depends on this one has finished, `afterDependents { }` runs the same way.

> **Pitfall.** Only public top-level `val`s are discovered. A `private val`, a patch inside an `object`, or a `fun` returning a patch is invisible to the engine, and a patch depending on one fails to load with `depends on a patch that is not declared as a public top-level value`.

> **Pitfall.** Top-level initialisers run at load time for the whole file. Code that touches the app there (calling `someTarget.method` outside `execute`) throws while the bundle loads; the engine logs `patch declaration failed to initialize` for the member and skips it, so a typo in one file can make several patches disappear from the list. Keep top-level code to declarations.

> **Pitfall.** Two patches with the same name in one bundle is an error before any patch runs. Names are identities.

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
- `execute { }`: the patch body. The receiver is the [runtime](7_runtime.md).
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

> **Pitfall.** Pinning versions means the patch is skipped on every other version, including ones where it would have worked. Leaving versions off means the patch runs everywhere and fails loudly when a target stops matching. Both are defensible; pin when a wrong match would do damage silently (a rewritten constant, a replaced body), leave open when a failed match is the worst case.

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

> **Pitfall.** A failed patch does not roll back what it already changed. Resolve every target you need before the first mutation when a half-applied patch would leave the app broken: reading `target.method` (or any property) forces resolution.

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

> **Pitfall.** Options are read inside `execute`, never in the patch block: the block runs at load time, before any user has set anything.

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

`toggle`, `text`, `folder`, `choice(title, default, choices = listOf(Choice(value, title)))`. Pass `key = "..."` to pin a key.

> **Pitfall.** The key is `<object>.<property>` in snake case (`telegram_settings.hide_sponsored_ads`) and is what the patched app stores the value under. Renaming the object or the property changes the key and resets the setting for every user. Pin `key =` before the first release.

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

`install` runs after every patch that registered settings with the host, once the host has written `assets/reseam/settings.json` for the runtime to read. Toggles gate emitted code; see [Gates](6_code.md#gates).

Next: [Finding code in the app](5_targets.md).
