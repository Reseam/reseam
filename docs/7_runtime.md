---
description: Edit the manifest, the resource table, and any file in the APK.
---

# Manifest, resources, and files

Besides bytecode, a patch can edit the manifest, the resource table, and any file in the APK. Inside `execute { }` they are `manifest`, `resources`, and `files`, next to `bytecode` for raw class lookup, `options`, and `log`.

![The patch runtime, receiver of execute and afterDependents, with six members: manifest, resources, files, bytecode, options, and log. Targets and code blocks resolve against the same runtime; manifest, resources, and files take component(name) for one split.](runtime-surface.svg)

```kotlin
manifest.addPermission("android.permission.INTERNET")
manifest.addActivity("app.example.SettingsActivity") { this["android:label"] = "Settings" }
manifest.edit {
    val application = findByTag("application").first()
    application.appendChild(createElement("meta-data").apply { this["android:name"] = "reseam" })
}

resources.addString("reseam_label", "Reseam")
val label = resources.id("string", "app_name")

files.write("assets/reseam/logo.png", bytes)
files.copy("resources/logo.png", "assets/logo.png")

bytecode.replaceAllStrings("com.example", newPackage)
bytecode.redirectCalls("android.telephony.TelephonyManager", "getDeviceId", Identity.deviceId)
log.info("renamed package")
```

`replaceAllStrings` matches a constant whole; when the value is embedded in a larger one, such as an authority inside a `content://` URI, use `bytecode.replaceStringsContaining(substring) { old -> ... }` and return null for the constants to leave alone.

`redirectCalls` sends every call to a method anywhere in the app to a static [extension](9_extensions.md) method instead, passing the receiver as the first argument for instance methods. Calls made from extension code keep their target, so the extension method can call the one it replaces. It is the primitive for patches that apply to any app: identity spoofing, blocking a library call, replacing a platform API. `Identity.deviceId` above is declared as `static("deviceId", "android.telephony.TelephonyManager", returns = Type.String)`.

`edit { }` opens the manifest as an XML document and closes it after the block; `files.editXml(path) { }` does the same for any XML entry. Every member is listed in the [reference](12_reference.md#runtime).

Compiled attributes need resource ids: `android:` resolves through the framework table, other declared namespaces through the app's `attr` entries. An unresolved name fails the patch. Unprefixed attributes such as `class` and `style` need no id. `declareNamespace(prefix, uri)` adds a namespace; like other structural edits, it shifts element indices.

> [!WARNING]
> Elements from `edit { }` are handles into a document that closes with the block. Take strings and ids out, never an `XmlElement`. Release builds often strip resource names, so `resources.id("string", "app_name")` can be null for a resource that exists; take the id from where the app refers to it (`resourceRef` on a manifest attribute) and work through `replaceEntry`.

## Combining XML

Compile a layout fragment from the patch jar and copy it into the app's layout:

```kotlin
XmlDocument.compile(fragment).use { source ->
    resources.editXml("layout", "player_controls") {
        root.appendChild(adopt(source.root))
    }
}
```

`XmlDocument.compile(text)` compiles XML text against the app's resource table, resolving `@type/name` references, the enum and flag names an attribute defines (`android:scaleType="center"`, `android:gravity="top|start"`), and the ids of attributes in any namespace the text declares. `@android:type/name` and `?android:attr/name` resolve through the public resource table of Android API level 36, and a name missing from it fails the compile. An unprefixed `@type/name` that the app's table does not define fails the compile too, so add a resource before the XML that references it. It is backed by no APK entry, so closing it discards it.

`adopt` returns a detached deep copy of an element of another document: strings interned here, and every attribute rebound to the id this document resolves it by. Namespaces match by uri, not by prefix, so a fragment written with `custom:` can enter a document using `app:` for the same namespace. A namespace the target does not declare fails the adoption.

## Resource files

Release builds can rename `res/layout/player_controls.xml` to `res/QYO.xml`. Reach the file through its resource name:

```kotlin
resources.editXml("layout", "player_controls") {
    findByTag("ImageView").first()["android:layout_marginBottom"] = "6dp"
}
val splash = resources.paths("drawable", "launch_screen")
```

`xml(type, name)` and `editXml(type, name) { }` open the default configuration's file, which is the one with no qualifier; `path(type, name)` returns that path and `paths(type, name)` every configuration's, the default first. An entry that is not file-backed, or whose value is not an entry of the APK, fails the patch.

`addFile` writes a file and registers it under a resource type:

```kotlin
val icon = resources.addFile("drawable", "reseam_icon", "res/reseam_icon.png", bytes)
for (density in listOf("hdpi", "xhdpi", "xxhdpi")) {
    resources.addFile("mipmap", "reseam_launcher", "res/mipmap-$density/reseam_launcher.png", png(density), density)
}
resources.addFile("mipmap", "reseam_launcher", "res/mipmap-anydpi-v26/reseam_launcher.xml", adaptiveIcon, "anydpi-v26")
```

The entry holds the file path under the requested type. The last argument is a configuration: empty for the default, a density (`xxhdpi`, `anydpi`, `420dpi`), `night` or `notnight`, and `vN`, joined with dashes. Other qualifiers fail the patch. Repeated calls with the same type and name add variants under one id, creating missing configurations. Use `addFile("drawable", ...)` for a drawable; `addString(name, path)` registers a string even when the path points at an image.

XML passed as `data` is compiled. Inline `<aapt:attr>` elements become sibling resources named `$name__N.xml`, with the same type and configuration; the parent references them through the named attribute. This lets an animated vector carry its vector and animators inline. Use `addFile` for inline resources; `files.write` lacks the resource type and name they need.

## Styles and arrays

`resources.arsc` holds a style or an array as a bag of items.

```kotlin
resources.style("Theme.App.Light") { this["headerIcon"] = "@drawable/reseam_header" }
resources.style("Theme.Reseam", parent = "@style/Theme.App.Light") {
    this["android:windowBackground"] = "@color/reseam_splash"
}
resources.setStringArray("double_tap_length_values", resources.getArray("double_tap_length_values") + "120")
```

An item name resolves the way a `<style>` item name does in resource XML: `android:name` through the framework table, an unprefixed name through the app's own `attr` entries. Values are read the way attribute values are, and a name that resolves to no id fails the patch. `setArray` parses typed literals; `setStringArray` writes every value as literal text, preserving numeric strings and strings starting with `@`. Both may change the element count; `getArray` reads the default configuration.

Style and array edits apply to every configuration defining the entry, including `values-night`. A new style is created in the default configuration and requires `parent`.

## Split APKs

```kotlin
for (component in manifest.components()) {
    manifest.component(component).edit { root["package"] = newPackage }
}
```

`manifest`, `resources`, and `files` take `component(name)` for one split; without it they act on the base. A package or version change must land in every component, or the split set will not install.

Next: [Reading obfuscated objects](8_bindings.md).
