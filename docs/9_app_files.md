---
description: Edit the manifest, resources, XML layouts, and any file in the APK.
---

# Manifest, resources, and files

Besides code, a patch can change the rest of the APK. Inside `execute { }`:

| | |
|---|---|
| `manifest` | `AndroidManifest.xml`: permissions, activities, package, version |
| `resources` | the app's resources: strings, colors, styles, layouts, images |
| `files` | any file in the APK |
| `bytecode` | every class, for lookups and app-wide replacements |
| `hermes` | the app's [Hermes JavaScript bundle](15_hermes.md) |
| `options` | the patch's [options](4_patches.md#options) |
| `log` | `info`, `warn`, and `debug` messages in the patch log |

## Manifest

```kotlin
manifest.addPermission("android.permission.INTERNET")
manifest.addActivity("app.example.SettingsActivity") { this["android:label"] = "Settings" }
manifest.setVersionName("2.14.0-patched")

manifest.edit {
    val application = findByTag("application").first()
    application["android:debuggable"] = "true"
}
```

`edit { }` opens the manifest as an XML document for anything the helpers don't cover.

## Resources

```kotlin
resources.addString("reseam_label", "Reseam")
val appName = resources.getString("app_name")
val id = resources.id("string", "app_name")

resources.editXml("layout", "player_controls") {
    findByTag("ImageView").first()["android:layout_marginBottom"] = "6dp"
}
```

Release builds often rename resource files (`res/layout/player_controls.xml` becomes `res/QYO.xml`), so reach them by resource type and name, as `editXml` does.

Add an image or other file as a resource:

```kotlin
resources.addFile("drawable", "reseam_icon", "res/reseam_icon.png", bytes)
resources.addFile("mipmap", "reseam_launcher", "res/mipmap-xxhdpi/reseam_launcher.png", png, "xxhdpi")
```

The last argument is the configuration: empty for the default, a density, `night` or `notnight`, or `vN`. XML passed this way is compiled.

Styles and arrays:

```kotlin
resources.style("Theme.App.Light") { this["headerIcon"] = "@drawable/reseam_icon" }
resources.setStringArray("speed_values", resources.getArray("speed_values") + "3.0")
```

> [!WARNING]
> Release builds often strip resource names too, so `resources.id("string", "app_name")` can return null for a resource that exists. Take the id from where the app refers to it instead.

## XML

`manifest.edit`, `resources.editXml`, and `files.editXml(path)` give you an `XmlDocument`: find elements by tag or attribute, read and set attributes, add, insert, and remove elements. The document closes when the block ends, so take values out of it, never the elements themselves.

To add your own layout, compile it and copy its elements in:

```kotlin
XmlDocument.compile(layoutXml).use { source ->
    resources.editXml("layout", "player_controls") {
        root.appendChild(adopt(source.root))
    }
}
```

Attribute names need resource ids: `android:` names resolve through Android's own table, other prefixes through the app's attributes. A name that resolves to nothing fails the patch.

## Files

```kotlin
files.write("assets/reseam/logo.png", bytes)
files.copy("resources/logo.png", "assets/logo.png")
files.delete("assets/tracking.json")
```

`files.copy` takes a file from your bundle's `resources/` folder; see [Bundle projects](13_bundles.md).

## App-wide replacements

```kotlin
bytecode.replaceAllStrings("com.example", newPackage)
bytecode.replaceStringsContaining("example.com") { old -> old.replace("example.com", "example.org") }
```

`replaceAllStrings` only matches whole strings. `replaceStringsContaining` lets you rewrite strings that contain a value; return null to leave one unchanged.

## Split APKs

Apps from the Play Store often come as a base APK plus split APKs for languages, screen densities, and CPU types. `manifest`, `resources`, and `files` act on the base APK unless you pick a split with `component(name)`:

```kotlin
for (name in manifest.components()) {
    manifest.component(name).edit { root["package"] = newPackage }
}
```

A package name or version change has to reach every split, or Android won't install the set.

Next: [Reading obfuscated objects](10_bindings.md).
