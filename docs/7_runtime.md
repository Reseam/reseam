# Manifest, resources, and files

Besides bytecode, a patch can edit the rest of the APK: the manifest, the resource table, and any file in the archive. Inside `execute { }` and `afterDependents { }` these are reachable as `manifest`, `resources`, and `files`, next to `bytecode` for raw class lookup, `options` for values the user set, and `log`.

![A card labelled the patch runtime, the receiver of execute and afterDependents, with six members: manifest (AndroidManifest.xml: addPermission, addActivity, edit), resources (resource table and string pool: addString, replaceEntry), files (every entry in the APK: read, write, copy, delete), bytecode (raw classes: findClass, classesExtending), options (values the user set for this patch), and log (shown by the CLI and Reseam Manager). Targets and code blocks resolve against the same runtime; manifest, resources, and files take component(name) for one split.](runtime-surface.svg)

`manifest`, `resources`, and `files` are split-APK aware: `component(name)` narrows the scope to a named split.

## Manifest

```kotlin
manifest.packageName
manifest.versionName
manifest.applicationClass
manifest.addPermission("android.permission.INTERNET")
manifest.setVersionName("2.0")
manifest.addActivity("app.example.SettingsActivity") {
    this["android:label"] = "Settings"
}
```

Read-only: `packageName`, `versionCode`, `versionName`, `minSdkVersion`, `splitName`, `applicationClass`.

Helpers: `addPermission`, `setVersionCode`, `setVersionName`, `setMinSdk`, `setAttributeInt(element, attr, value)`, `setAttributeString`, `setActivityConfigChanges`, `addIntentFilter`, `addActivityAlias`, `copyIntentFilters`. `addActivity` skips an activity the manifest already declares and leaves it unexported unless the block says otherwise.

`edit { }` opens the manifest as an [XML document](#xml) and closes it after the block.

> **Pitfall.** Elements from `edit { }` are handles into a document that closes when the block returns. Keep the work inside the block; take plain values (a string, a resource id) out of it, never an `XmlElement`.

## Resources

```kotlin
val id = resources.id("string", "app_name")
resources.setString("app_name", "Example")
resources.addString("reseam_label", "Reseam")
```

- `id(type, name)`, `exists(type, name)`, `getString(name)`, `setString(name, value)`.
- `add(type, name, value)`, `addString`, `addBool`, `addInteger`, `addColor`, `addDimen`, `addId`, `addRaw`, `getRaw`.
- `owningComponent(type, name)`, `owningComponent(resId)`: which split defines a resource.
- `poolGet`, `poolSet`, `poolAdd`, `poolFindRefs`, `replaceEntry(resId, poolIndex)`: the string pool, for apps that strip resource names.

> **Pitfall.** Release builds often strip resource names, so `id("string", "app_name")` returns null even though the resource exists. Take the id from where the app refers to it (the manifest attribute, via `resourceRef`) and work through `replaceEntry`, as the Instagram clone patch does.

## Files

```kotlin
files.write("assets/reseam/logo.png", bytes)
files.copy("resources/logo.png", "assets/logo.png")
val manifest = files.read("AndroidManifest.xml")
```

- `list()`, `read(path)`, `write(path, bytes)`, `writeStored(path, bytes)` (no compression), `delete(path)`.
- `copy(bundlePath, apkPath)`: a file shipped in the bundle into the APK.
- `source()`: the component's original bytes. `signers()`: the original signer certificates.
- `xml(path)`: open an XML entry. `editXml(path) { }`: open and close it around the block.

## XML

```kotlin
manifest.edit {
    val application = findByTag("application").first()
    val activity = createElement("activity").apply {
        this["android:name"] = "app.example.MainActivity"
        setResourceRef("android:theme", themeId)
    }
    application.appendChild(activity)
}
```

A document has `root`, `findByTag`, `findByAttribute(name, value)`, `createElement`. An element has `tag`, `parent`, `children`, `get` and `set` for attributes, `setInt`, `setBool`, `setResourceRef`, `removeAttribute`, `appendChild`, `insertBefore`, `remove`, `clone`. `resourceRef("@0x7f1400a0")` parses a resource reference attribute into an id.

## Bytecode

```kotlin
bytecode.replaceAllStrings("com.example", newPackage)
val app = bytecode.findClass("com.example.App")
bytecode.classesExtending(Type.Application)
```

`classes` lists every class. These return [raw bytecode layer](10_dex.md) handles; targets are the usual way in.

## Options and log

`options[option]` and `options.getOrNull(option)`. See [Options](4_patches.md#options).

`log.info`, `log.warn`, `log.debug`. Entries are attributed to the running patch and shown by the CLI and Reseam Manager. Resolved targets are logged at debug level.

## Split APKs

```kotlin
for (component in manifest.components()) {
    manifest.component(component).edit { root["package"] = newPackage }
}
resources.component("config.xxhdpi").exists("drawable", "icon")
files.component("config.en").write("assets/marker.txt", bytes)
```

`components()` lists the base and every split. Without `component(...)`, operations run against the base.

> **Pitfall.** The platform refuses to install a split set whose APKs disagree on the package name or version. A change to either must be made in every component, as in the loop above, not in the base alone.

Next: [Reading obfuscated objects](8_bindings.md).
