---
description: Edit the manifest, resources, XML layouts, and any file in the APK.
---

# Manifest, resources, and files

Besides code, a patch can change everything else in the APK. Inside `execute { }` you have:

| | |
|---|---|
| `manifest` | `AndroidManifest.xml`: permissions, activities, package, version |
| `resources` | the app's resources: strings, colors, styles, layouts, images |
| `files` | any file in the APK |
| `bytecode` | every class, for lookups and app-wide replacements |
| `hermes` | the app's [Hermes JavaScript bundle](14_hermes.md) |
| `options` | the patch's [options](4_patches.md#options) |
| `log` | messages in the patch log: `info`, `warn`, `debug` |

## Manifest

```kotlin
manifest.addPermission("android.permission.INTERNET")
manifest.addActivity("app.example.SettingsActivity") { this["android:label"] = "Settings" }
manifest.setVersionName("2.14.0-patched")
```

For anything the helpers don't cover, `edit { }` opens the manifest as an [XML document](#xml):

```kotlin
manifest.edit {
    val application = findByTag("application").first()
    application["android:debuggable"] = "true"
}
```

[`ManifestScope`](reference_files.md#manifestscope) lists every helper, including intent filters and activity aliases.

## Mount builds

On a rooted phone, a build can be *mounted* over the app installed from the store, instead of installed as an app of its own. The app keeps its signature, its data, and Google sign-in.

Android keeps reading the installed app's manifest, so a mount build leaves out every patch that edits the manifest, and the patches that depend on it. Code, resources, and other files apply as usual. A patch that [rewrites the written APK](#after-the-apk-is-written) is left out too.

Where code and resource changes can do the job, use them instead of manifest changes, so your patch works both ways. A patch whose whole purpose is the manifest, such as a package rename, is left out of mount builds.

## Resources

```kotlin
resources.addString("reseam_label", "Reseam")
val appName = resources.getString("app_name")
val id = resources.id("string", "app_name")
```

Release builds often rename resource files, so `res/layout/player_controls.xml` becomes something like `res/QYO.xml`. Reach a file by its resource type and name instead of its path:

```kotlin
resources.editXml("layout", "player_controls") {
    findByTag("ImageView").first()["android:layout_marginBottom"] = "6dp"
}
```

> [!WARNING]
> Release builds often strip resource names too, so `resources.id("string", "app_name")` can return null for a resource that exists. When that happens, take the id from where the app refers to it, such as an attribute in the manifest or a layout.

### Adding files as resources

Add an image or other file as a resource:

```kotlin
resources.addFile("drawable", "reseam_icon", "res/reseam_icon.png", bytes)
resources.addFile("mipmap", "reseam_launcher", "res/mipmap-xxhdpi/reseam_launcher.png", png, "xxhdpi")
```

The last argument is the configuration: empty for the default, a density, `night` or `notnight`, or `vN`. XML passed this way is compiled.

### Styles and arrays

```kotlin
resources.style("Theme.App.Light") { this["headerIcon"] = "@drawable/reseam_icon" }
resources.setStringArray("speed_values", resources.getArray("speed_values") + "3.0")
```

[`ResourceScope`](reference_files.md#resourcescope) lists the rest, including the string pool functions for changing one string everywhere it is used.

## XML

`manifest.edit`, `resources.editXml`, and `files.editXml(path)` each give you an `XmlDocument`. You can find elements by tag or attribute, read and set attributes, and add, move, or remove elements.

Your changes are written back when the block ends. The document closes then too, so take values out of it, never the elements themselves.

Attribute names need resource ids, which is how Android reads them. `android:` names resolve through Android's own attributes, and other prefixes through the app's. A name that resolves to nothing fails the patch, since Android would ignore it.

### Adding your own layout

To add elements of your own, compile them, then adopt them into the document:

```kotlin
XmlDocument.compile(layoutXml).use { source ->
    resources.editXml("layout", "player_controls") {
        root.appendChild(adopt(source.root))
    }
}
```

## Files

```kotlin
files.write("assets/reseam/config.json", bytes)
files.copy("resources/logo.png", "assets/logo.png")
files.delete("assets/tracking.json")
```

`files.copy` takes a file from your bundle's `resources/` folder, which [Bundle projects](15_bundles.md#layout) describes.

### Large files

`files.read` and `files.write` hold the whole file in memory. A large file, such as a native library or a bundled model, may not fit, especially when patching on a phone. Stream it instead:

```kotlin
files.stream("lib/arm64-v8a/libapp.so")?.use { input ->
    val header = ByteArray(64)
    DataInputStream(input).readFully(header)
    log.info("ELF class ${header[4]}")
}
```

`files.write` and `files.writeStored` also take an `InputStream`, and write everything it reads. A stream reads the file as it was when it opened, so you can write the file you are reading:

```kotlin
files.stream("assets/catalog.bin")?.use { input ->
    files.write("assets/catalog.bin", InflaterInputStream(input))
}
```

Close every stream before your block ends, which `use { }` does for you. If reading the stream you pass to `write` fails, the file stays as it was.

`files.sourceStream()` reads the whole original APK. To read one file of it, use `files.stream`.

## App-wide replacements

```kotlin
bytecode.replaceAllStrings("com.example", newPackage)
bytecode.replaceStringsContaining("example.com") { old -> old.replace("example.com", "example.org") }
bytecode.replaceStringsStartingWith("content://com.example") { old -> old.replace("com.example", newPackage) }
```

`replaceAllStrings` only matches whole strings. `replaceStringsContaining` rewrites strings that contain a value: return the new string, or null to leave one as it is. `replaceStringsStartingWith` does the same for strings that start with a value. Each returns how many strings changed.

## Split APKs

Apps from the Play Store often come as a base APK plus split APKs for languages, screen densities, and CPU types. `manifest`, `resources`, and `files` work on the base APK, unless you pick a split with `component(name)`:

```kotlin
for (name in manifest.components()) {
    manifest.component(name).edit { root["package"] = newPackage }
}
```

A package name or version change has to reach every split, or Android won't install the set.

## After the APK is written

Some work needs the finished APK file instead of its parts, such as handing it to a tool that takes an APK. `afterWrite { }` runs once every patch has executed and the engine has written the APK, just before it is signed:

```kotlin
val packApp = patch("Pack app") {
    compatibleWith(EXAMPLE_APP)

    afterWrite {
        val packed = tempFile("base.apk")
        pack(output.file, packed, config = bundleFile("resources/packer.json"))
        output.replace(packed)
    }
}
```

`pack` stands for your own code, or a library your patch module depends on.

| | |
|---|---|
| `output.file` | the base APK as written, unsigned |
| `output.component(name)` | a split, by the names `files.components()` lists |
| `output.replace(file)` | moves `file` over the APK. The engine signs whatever is in place at the end. |
| `bundleFile(path)` | a file from your bundle, at the path `files.copy` takes |
| `tempFile(name)` | a new path the engine deletes after the run, on the same disk as the output |
| `options`, `log` | as in `execute` |

`manifest`, `resources`, `bytecode`, and targets aren't available here, because the engine has already written them out. Patches with `afterWrite` run in dependency order, each on the files the one before it left.

A patch that fails here fails the whole run, since the output is already written. Mount builds leave these patches out, and the browser patcher can't run them.

Next: [Testing and debugging](9_testing.md).
