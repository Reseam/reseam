---
description: Name a patch, say which apps it is for, and give it options and dependencies.
---

# Declaring patches

A patch is a public top-level `val` built with `patch("Name") { }`. The block declares what users and the engine need to know, then `execute { }` holds the change.

```kotlin
val hideAds = patch("Hide ads") {
    description("Removes ads from the feed.")
    compatibleWith("com.example.app"("2.14.0"))

    execute {
        showAd.before { returnVoid() }
    }
}
```

> [!WARNING]
> The engine only finds public top-level values. A `private val`, or a patch inside an `object`, is invisible.

## Names and IDs

The name is what users see; it doesn't have to be unique. Each patch also has an ID taken from its Kotlin declaration (`package.property`), and a *reference*, `<bundle>/<id>`, where the bundle name comes from `manifest.toml`. Renaming the property or its package changes the ID; changing the display name does not.

## Which apps

```kotlin
compatibleWith("com.example.app", "com.example.app.lite")
compatibleWith("com.example.app"("2.14.0", "2.14.1"))
```

A package on its own means every version. `"package"("version", ...)` limits the patch to those versions; on any other version it is skipped. Define the package once and reuse it: `val EXAMPLE_APP = "com.example.app"("2.14.0")`.

A patch without `compatibleWith` works on any app. Users have to pick it themselves; it is never selected by default.

> [!WARNING]
> Pinned versions skip the patch even on versions where it would work. Unpinned, it runs everywhere and fails loudly when its targets stop matching. Pin versions when a wrong match could do damage quietly, such as rewriting a constant.

## On or off by default

Patches for a specific app start switched on. `enabledByDefault(false)` makes one opt-in.

## Internal patches

`patch { }` without a name is internal: users never see it, and it only runs when a patch that runs depends on it. Use it for setup that several patches share.

```kotlin
val adBlockerSetup = patch {
    compatibleWith(EXAMPLE_APP)
    execute { appEntry { call(AdBlocker.init, application) } }
}
```

`hidden()` keeps a named patch off the lists users see.

## Dependencies

```kotlin
val hideAds = patch("Hide ads") {
    compatibleWith(EXAMPLE_APP)
    dependsOn(adBlockerSetup)
    execute { /* ... */ }
}
```

A dependency runs first. If it is skipped or fails, the patches that depend on it are skipped.

### Patches from another bundle

To depend on a patch someone else publishes, declare their bundle in your patch module's `build.gradle.kts`:

```kotlin
reseam {
    bundle(index = "https://patches.example.com/patches.json", version = "1.4.0")
}
```

Use `bundle(file = file("../other/build/reseam/other.reseam"))` for a bundle you are building next to yours, and `signer = "<public key>"` on `bundle(index = ...)` to pin the key you expect.

The build downloads that bundle and generates a Kotlin value for each of its patches, in the same package as the original:

```kotlin
import com.example.other.removeProtection

val hideBanners = patch("Hide banners") {
    dependsOn(removeProtection)
    execute { /* ... */ }
}
```

Users need to load that bundle too. If it is missing, your patch is skipped with a message naming the bundle.

## Options

Options are values users set before patching, such as a new app name.

```kotlin
val cloneApp = patch("Clone app") {
    compatibleWith(EXAMPLE_APP)
    val packageName = stringOption("packageName", title = "Package name", default = "com.example.app.clone")

    execute {
        val newPackage = options[packageName]
    }
}
```

| Declaration | Value in Kotlin |
|---|---|
| `stringOption` | `String`, with optional `validValues` |
| `boolOption` | `Boolean` |
| `intOption` | `Long` |
| `floatOption` | `Double` |
| `stringListOption` | `List<String>` |
| `pathOption` | `OptionPath`: a file or folder the user picks |

Each takes `key`, and optionally `title`, `description`, `default` (not for paths), and `required`. `options[x]` returns the value and throws if there is none; `options.getOrNull(x)` returns null instead. From the CLI, set one with `--option <patch>.<key>=<value>`.

Next: [Settings inside the app](5_settings.md).
