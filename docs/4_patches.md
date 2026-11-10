---
description: Name a patch, say which apps it is for, and give it options and dependencies.
---

# Declaring patches

A patch is a public top-level `val` built with `patch(name) { }`. The block first says what users and the engine need to know about the patch, then `execute { }` holds the change.

```kotlin
val hideAds = patch(ExamplePatchStrings.hideAds) {
    description(ExamplePatchStrings.hideAdsDescription)
    compatibleWith("com.example.app"("2.14.0"))

    execute {
        showAd.before { returnVoid() }
    }
}
```

> [!WARNING]
> The engine only finds public top-level values. A `private val`, or a patch declared inside an `object`, is invisible to it.

## Names and IDs

The name is what users see, and it doesn't have to be unique. Each patch also has an ID, taken from its Kotlin declaration: `package.property`, such as `app.example.patches.hideAds`. Its *reference* adds your bundle's name from `manifest.toml`: `my-patches/app.example.patches.hideAds`.

Renaming the property or moving it to another package changes the ID. Users who saved a selection, and bundles that depend on your patch, refer to it by ID, so keep IDs stable once you publish. Changing the display name is safe.

## Names in every language

The name, the description, and option titles and descriptions are strings from the patch module's string table, so translators can work on them without touching code. Put them in `src/main/strings/values/strings.xml`, and name the object the build generates in the module's `build.gradle.kts`:

```xml
<resources>
    <string name="hide_ads">Hide ads</string>
    <string name="hide_ads_description">Removes ads from the feed.</string>
</resources>
```

```kotlin
reseamStrings { objectName = "app.example.patches.ExamplePatchStrings" }
```

A translation is a `strings.xml` in a folder named for its locale, such as `values-de` or `values-pt-rBR`. It can translate some strings and leave the rest, which then show in the source language. When you build the bundle, the engine copies every translation into the signed catalog. Reseam Manager shows each patch in the user's language, and the CLI keeps the source text, which is also what `--enable`, `--disable` and `--option` match.

Patch text has to be plain: a string with a placeholder, or a `<plurals>`, isn't in the generated object, so the build fails if a patch names one. Mark a string that must stay the same in every language, such as a host name, with `translatable="false"`. The build then rejects any translation of it.

## Which apps

```kotlin
compatibleWith("com.example.app", "com.example.app.lite")
compatibleWith("com.example.app"("2.14.0", "2.14.1"))
compatibleWith("com.example.app"("2.14.0", 214001))
compatibleWith(CompatiblePackage("com.example.app", versionCodes = listOf(214001, 214002)))
```

- A package on its own means every version.
- `"package"("2.14.0", "2.14.1")` limits the patch to those version names.
- `"package"("2.14.0", 214001)` names one version by its name and its `android:versionCode`. Either one matching is enough.
- `versionCodes` limits the patch to those codes. Use it when several builds share a version name.

On any other version, the patch is skipped. Users can run it anyway, for example with `--ignore-versions` on the CLI.

Define each package once and reuse it:

```kotlin
val EXAMPLE_APP = "com.example.app"("2.14.0")
```

> [!WARNING]
> Pinning versions is a trade-off. A pinned patch is skipped on versions where it would still work. An unpinned one runs everywhere, and fails loudly when its targets stop matching. Pin versions when a wrong match could do harm quietly, such as a patch that rewrites a number.

### Patches for any app

A patch without `compatibleWith` works on any app, such as one that allows screenshots. Since it can run on anything, Reseam doesn't select it every time someone patches an app. It starts switched off, and users turn it on themselves.

If most users would want it, call `enabledByDefault(true)`. It is then selected along with the app's own patches.

## On or off by default

A patch for a specific app starts selected. Call `enabledByDefault(false)` to make it opt-in, for a change that only some users want.

## Internal and hidden patches

`patch { }` without a name is an internal patch. Users never see it, and it only runs when a patch that runs depends on it. Use it for setup that several patches share:

```kotlin
val adBlockerSetup = patch {
    compatibleWith(EXAMPLE_APP)
    execute { appEntry { AdBlocker.init(application) } }
}
```

`hidden()` keeps a named patch off the lists users see. It still runs when something depends on it, or when a user enables it by name.

## Dependencies

```kotlin
val hideAds = patch(ExamplePatchStrings.hideAds) {
    compatibleWith(EXAMPLE_APP)
    dependsOn(adBlockerSetup)
    execute { /* ... */ }
}
```

A dependency runs first. If it is skipped or fails, the patches that depend on it are skipped too. Turning a patch on also turns on what it depends on.

### Patches from another bundle

To depend on a patch that someone else publishes, declare their bundle in your patch module's `build.gradle.kts`:

```kotlin
reseam {
    bundle(index = "https://patches.example.com/patches.json", version = "1.4.0")
}
```

Add `signer = "<public key>"` to make the build check that the bundle is signed with the key you expect. For a bundle you build next to yours, use `bundle(file("../other/build/reseam/other.reseam"))`.

The build downloads that bundle and generates a Kotlin value for each of its patches, in the same package as the original:

```kotlin
import com.example.other.removeProtection

val hideBanners = patch(ExamplePatchStrings.hideBanners) {
    dependsOn(removeProtection)
    execute { /* ... */ }
}
```

Users need to load that bundle too. If it is missing, your patch is skipped, with a message naming the bundle.

## Options

Options are values users choose before patching, such as a new app name. Declare one in the patch block, and read it in `execute`:

```kotlin
val cloneApp = patch(ExamplePatchStrings.cloneApp) {
    compatibleWith(EXAMPLE_APP)
    val packageName =
        stringOption("packageName", title = ExamplePatchStrings.packageName, default = "com.example.app.clone")

    execute {
        val newPackage = options[packageName]
    }
}
```

| Declaration | Value in Kotlin | Notes |
|---|---|---|
| `stringOption` | `String` | `validValues` limits it to a list. |
| `boolOption` | `Boolean` | |
| `intOption` | `Long` | |
| `floatOption` | `Double` | |
| `stringListOption` | `List<String>` | |
| `pathOption` | `OptionPath` | A folder the user picks. `listContents()` and `readFile(path)` read it. |

Each takes a `key` and a `title`, and optionally a `description`, a `default` (except paths), and `required`. The title and description are strings from the module's string table, like the patch name. The key is how users set the option, for example `--option "Clone app.packageName=com.example.clone"` on the CLI.

`options[x]` returns the value, with the default filled in. It fails when the option is optional, has no default, and the user set nothing. Read such an option with `options.getOrNull(x)`, which returns null instead.

The engine checks every value before any patch runs: its type, that a text value is one of `validValues`, and that a path exists. Your patch only ever sees valid values.

## When each block runs

A patch can have up to three blocks. Most patches only need `execute`.

| Block | Runs | Use it for |
|---|---|---|
| `execute { }` | once all its dependencies have run | changing code, resources, and files |
| `afterDependents { }` | after every patch that depends on it has finished | work that needs what those patches set up |
| `afterWrite { }` | after the APK is written, before it is signed | tools that need the finished file |

`afterDependents` exists for patches that other patches build on. A settings host is the usual case. Each patch that adds settings registers them with the host while it runs, and the host can only build the screen once all of them are done.

`afterWrite` gets the finished APK file instead of the app's parts. See [After the APK is written](8_app_files.md#after-the-apk-is-written).

### Collecting values from other patches

For your own shared patches, collect what dependents register in a `RunList`, and read it in `afterDependents`:

```kotlin
private val feedFilters = RunList<ExtMethod>()

fun filterFeed(filter: ExtMethod) = feedFilters.add(filter)

val feedFiltering = patch {
    compatibleWith(EXAMPLE_APP)
    afterDependents {
        loadFeed.after { feedFilters.values.forEach { call(it, thisObject.field(feedItems)) } }
    }
}
```

A dependent calls `filterFeed(...)` in its `execute`. Use a `RunList` rather than a top-level `mutableListOf()`. A plain list outlives the run, and keeps values from a patch that failed and was undone. A `RunList` starts empty on every run, and forgets the values of an undone patch.

## When a patch fails

Reseam undoes everything the failed patch changed, and carries on with the rest. Patches that depend on it are skipped. At the end, Reseam Manager and the CLI report each patch as applied, skipped, or failed.

By default, a run with a failed patch writes no APK. That way, nobody gets an app that's missing a change they asked for. On the CLI, `--leave-out-failed` writes the APK anyway, without the failed patches and the patches that need them.

Next: [Finding code](5_finding_code.md).
