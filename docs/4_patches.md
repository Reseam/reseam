# Patches

A patch is one change to one app that a user can switch on: hide ads, unlock a paid feature, remove an update prompt. Reseam applies it by rewriting the APK on the phone; the patched app installs next to the original.

In code, a patch is a public top-level `val` in `apps/<app>/patch/src/main/kotlin/`. It carries what users and the engine need up front (name, description, apps, dependencies, options) and an `execute` block with the change. The engine finds patches by scanning the compiled classes, so file and property names are yours.

```kotlin
import app.reseam.patch.Type
import app.reseam.patch.invoke
import app.reseam.patch.method
import app.reseam.patch.patch
import app.reseam.patch.settings.section
import app.reseam.patch.settings.skipWhen

val hideAds = patch("Hide ads") {
    description("Removes ads from the feed.")
    compatibleWith("com.example.app"("2.14.0"))
    settings(appSettings, section("Ads", AppSettings.hideAds))

    execute {
        showAd.skipWhen(AppSettings.hideAds)
    }
}

val showAd = method("showAd") {
    strings("ad_impression")
    returns(Type.Void)
}
```

The patch comes first, its [targets](5_targets.md) below it. Everything the block accepts is in the [reference](12_reference.md#declaring-patches).

## How a patch runs

1. **Load.** The engine opens the bundle and initialises every top-level value in the patch jar, reading metadata off the patches. Nothing has looked at the app. A target is only a description; `.method`, `options[...]` and every scope throw `This API is only available while a patch is executing`.
2. **Selection.** A patch is skipped when its package or version does not match the APK, when a dependency was skipped, or when the user left it off.
3. **Patch time.** `execute { }` runs once with the runtime as receiver. Targets resolve on first use and stay cached for this patch. Code blocks run immediately and emit instructions. `afterDependents { }` runs after every dependent has finished.

An uncaught exception fails the patch, skips its dependents, and does not roll back what it already changed.

> [!WARNING]
> Only public top-level `val`s are discovered. A `private val` or a patch inside an `object` is invisible, and a dependency on one fails with `depends on a patch that is not declared as a public top-level value`. Top-level code that touches the app throws at load and the engine skips that file's patches with `patch declaration failed to initialize`.

## Internal patches and dependencies

`patch { }` without a name is internal: never listed, never selected, run whenever an enabled patch depends on it. Use it for shared setup.

```kotlin
val adBlockerRuntime = patch {
    compatibleWith(EXAMPLE_APP)

    execute {
        appEntry.before { call(AdBlocker.init, thisObject) }
    }
}

val hideAds = patch("Hide ads") {
    compatibleWith(EXAMPLE_APP)
    dependsOn(adBlockerRuntime)

    execute { }
}
```

`dependsOn` takes references, never names. A dependency runs first; skipping it skips its dependents.

## Depending on another bundle

A patch can depend on a patch from a bundle someone else publishes. Declare that bundle in the module's build script, by its `patches.json` and the version you build against, or by a file when it is a bundle you are developing next door:

```kotlin
reseam {
    bundle(index = "https://api.reseam.app/patches.json", version = "1.4.0")
    bundle(file = file("../other-bundle/build/reseam/other-bundle.reseam"))
}
```

The build fetches the bundle and generates a reference for each of its patches, in the same package as the original, so the dependency reads like a local one:

```kotlin
import app.reseam.patches.universal.removeProtection

val hideBanners = patch("Hide banners") {
    compatibleWith(EXAMPLE_APP)
    dependsOn(removeProtection)

    execute { }
}
```

A wrong name is a compile error, and when the other bundle moves or renames a patch, your build breaks when you bump the version instead of a user's run. `signer = "<public key hex>"` on `bundle(index, ...)` pins the key the index must carry; fetching a bundle loads its code, so it is checked against the index's key either way.

The user loads that bundle alongside yours. When it is missing, the patch is skipped with `depends on other-bundle/…; load bundle 'other-bundle' alongside`, and selecting it explicitly fails with the same message.

## Compatibility

```kotlin
compatibleWith("com.example.app", "com.example.app.lite")
compatibleWith("com.example.app"("2.14.0", "2.14.1"))
```

A package alone means every version; `"package"("version", ...)` pins versions. One patch can cover several apps. Define the package once and share it across the bundle: `val EXAMPLE_APP = "com.example.app"("2.14.0")`. A patch with no `compatibleWith` is universal: it applies to every app, and because the engine cannot know it suits an arbitrary app, it is off until the user selects it. Declare `enabledByDefault(true)` to override that.

> [!WARNING]
> Pinned versions skip the patch on every other version, including ones where it would work. Unpinned, it runs everywhere and fails loudly when a target stops matching. Pin when a wrong match does damage silently (a rewritten constant, a replaced body).

## Options

Values the user sets when applying the patch. Declare them in the block, read them in `execute`:

```kotlin
val cloneApp = patch("Clone app") {
    compatibleWith(EXAMPLE_APP)
    val packageName = stringOption("packageName", title = "Package name", default = "com.example.app.clone")

    execute {
        val newPackage = options[packageName]
    }
}
```

`options[x]` applies defaults and throws for an empty optional option; `options.getOrNull(x)` returns null. From the CLI: `--option <patch>.<key>=<value>`.

## Settings

Switches inside the patched app, shown by a settings screen a host installs. Declare them as properties; the key derives from the object and property names:

```kotlin
object AppSettings {
    val hideAds by toggle("Hide ads", default = true)
    val unlockFeatures by toggle("Unlock features", summary = "Server-checked features still need a subscription.", default = true)
}

val appSettings = settingsHost("example") {
    compatibleWith(EXAMPLE_APP)

    install {
        appEntry.before { call(SettingsEntry.init, thisObject) }
        manifest.addActivity("app.example.ext.settings.ReseamSettingsActivity") {
            this["android:label"] = "Reseam Settings"
        }
    }
}
```

`settings(host, section(...))` in a patch registers its sections and adds the host as a dependency. `install` runs after every registering patch, once the host has written `assets/reseam/settings.json`. Toggles gate emitted code; see [Gates](6_code.md#gates).

> [!WARNING]
> The key (`app_settings.hide_ads`) is what the app stores the value under. Renaming the object or property resets the setting for every user. Pin `key =` before the first release.

Next: [Finding code in the app](5_targets.md).
