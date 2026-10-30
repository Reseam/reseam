---
title: "API: Manifest, resources, and files"
description: Reading and editing the manifest, compiled resources, XML documents, and any file in the APK.
---

# API: Manifest, resources, and files

The parts of the APK besides code. The guide page is [Manifest, resources, and files](8_app_files.md). Everything here is in `app.reseam.patch`.

`manifest`, `resources`, and `files` all work on the base APK. For an app that ships as split APKs, `components()` lists every APK, and `component(name)` gives you the same scope for one of them.

## Manifest

### `ManifestScope`

The `manifest` of a running patch.

**Reading**

| Member | Description |
|---|---|
| `packageName: String?` | The package name. |
| `versionCode: Int?` | `android:versionCode`. |
| `versionName: String?` | `android:versionName`. |
| `minSdkVersion: UInt?` | The minimum Android version, as an API level. |
| `splitName: String?` | This APK's split name, or null for the base APK. |
| `applicationClass: String?` | The `Application` class's full name, or null when the app uses Android's default. |
| `components(): List<String>` | The names of every APK in the app. |
| `component(name: String): ManifestScope` | The manifest of another APK. |

**Changing**

| Member | Description |
|---|---|
| `setVersionCode(code: Int)` | Sets `android:versionCode`. |
| `setVersionName(name: String)` | Sets `android:versionName`. |
| `setMinSdk(sdk: UInt)` | Sets the minimum Android version. |
| `addPermission(permission: String)` | Adds a `<uses-permission>`. It doesn't check whether the manifest already declares it. |
| `setAttributeInt(elementName: String, attrName: String, value: Int)` | Sets an `android:` attribute of the first element with this tag to a number. |
| `setAttributeString(elementName: String, attrName: String, value: String)` | Sets an `android:` attribute of the first element with this tag. `@type/name` and `?attr` become resource references, and enum and flag names (`singleTask`, `orientation\|screenSize`) become their values, as aapt writes them. Other text stays text. Start with `\@` or `\?` to keep the character. |
| `setActivityConfigChanges(activityName: String, configChanges: String)` | Sets an activity's `android:configChanges`. |
| `addIntentFilter(activityName: String, action: String? = null, category: String? = null, mimeType: String? = null)` | Adds an `<intent-filter>` to an activity. |
| `addActivityAlias(targetActivity: String, aliasName: String, enabled: Boolean = true, label: String? = null)` | Adds an `<activity-alias>` for an activity, after the activities, where Android requires it. `label` is read like `setAttributeString`, so `@string/name` is a reference. |
| `copyIntentFilters(fromActivity: String, toActivity: String)` | Copies every intent filter from one activity to another. Either can be an alias. |
| `addActivity(name: String, block: XmlElement.() -> Unit = {})` | Declares an activity, not exported, unless the manifest already declares it. The block sets more attributes. |
| `edit(block: XmlDocument.() -> T): T` | Opens the manifest as an [`XmlDocument`](#xmldocument) for anything else, and closes it when the block ends. |
| `document(): XmlDocument` | Opens the manifest without closing it. Close it yourself, with `use { }`. |

A [mount build](8_app_files.md#mount-builds) leaves out every patch that changes the manifest.

### `resourceRef`

`resourceRef(value: String): UInt?` reads a reference attribute value such as `@0x7f1400a0` or `@ref/0x7f1400a0` as a resource id. Other text gives null.

## Resources

### `ResourceScope`

The `resources` of a running patch: the app's compiled resource table. Lookups of a name that doesn't exist return null or `false`.

**Finding resources**

| Member | Description |
|---|---|
| `id(resType: String, resName: String): UInt?` | A resource's id, such as `id("string", "app_name")`. |
| `exists(resType: String, resName: String): Boolean` | Whether the resource exists. |
| `owningComponent(resType: String, resName: String): String?` | The name of the APK that defines the resource. |
| `owningComponent(resId: UInt): String?` | The same, by id. |
| `components(): List<String>` | The names of every APK in the app. |
| `component(name: String): ResourceScope` | The resources of another APK. |

**Values**

| Member | Description |
|---|---|
| `getString(name: String): String?` | A string's text. |
| `getString(resId: UInt): String?` | A string's text, by the id an attribute references. |
| `setString(name: String, value: String): Boolean` | Changes a string's text. Returns false when there is no such string. |
| `setString(resId: UInt, value: String): Boolean` | The same, by id. |
| `add(resType: String, name: String, value: String): UInt?` | Adds or replaces a value resource in the default configuration. Returns its id. |
| `add(resType: String, name: String, value: String, qualifiers: String): UInt?` | The same, in the configuration `qualifiers` names, written as in `addFile`. |
| `addString(name: String, value: String): UInt?` | Adds or replaces a string. |
| `addBool(name: String, value: Boolean): UInt?` | Adds or replaces a boolean. |
| `addInteger(name: String, value: Int): UInt?` | Adds or replaces an integer. |
| `addColor(name: String, color: String): UInt?` | Adds or replaces a color such as `#FF6200EE`. |
| `addColor(name: String, color: String, qualifiers: String): UInt?` | The same, in one configuration, such as `night`. |
| `addDimen(name: String, dimen: String): UInt?` | Adds or replaces a dimension such as `16dp`. |
| `addId(name: String): UInt?` | Adds an `id` resource, as `@+id/name` does. |
| `addRaw(resType: String, name: String, dataType: UByte, data: UInt): UInt?` | Adds a value with an Android type byte and its raw bits, for types the helpers don't cover. |
| `getRaw(resType: String, resName: String): Long?` | A value's raw bits. |
| `setPackageName(name: String)` | Renames the resource table's package. An app installed under a new package needs this, because Android's by-name lookups match the table's package name. |

**Files and XML**

Release builds often rename resource files, so `res/layout/player_controls.xml` can become `res/QYO.xml`. Reach them by type and name, never by path.

| Member | Description |
|---|---|
| `path(resType: String, resName: String): String` | The APK path of the default configuration's file. Fails when the resource is missing or isn't a file. |
| `paths(resType: String, resName: String): List<String>` | The paths of every configuration's file, the default first. |
| `xml(resType: String, resName: String): XmlDocument` | Opens the default configuration's file. Close it yourself, with `use { }`. |
| `editXml(resType: String, resName: String, block: XmlDocument.() -> T): T` | Opens the file, runs the block, and closes it. |
| `addFile(resType: String, name: String, apkPath: String, qualifiers: String = ""): UInt` | Registers a file already in the APK as a resource. Returns its id. |
| `addFile(resType: String, name: String, apkPath: String, data: ByteArray, qualifiers: String = ""): UInt` | Writes the file, then registers it. XML is compiled, and each `<aapt:attr>` in it becomes a resource of its own, as aapt builds them. |

`qualifiers` is written as in a `res/` folder name: `""` for the default, a density such as `xxhdpi` or `anydpi-v26`, `night`, `notnight`, or `vN`. Every configuration of one resource uses the same `name`.

**Styles and arrays**

| Member | Description |
|---|---|
| `style(name: String, parent: String? = null, block: StyleScope.() -> Unit): UInt` | Adds or replaces items in every configuration of a style. Creating a style needs `parent`. On an existing style, a `parent` replaces its parent. |
| `getArray(name: String): List<String>` | An array's elements as text. |
| `setArray(name: String, values: List<String>): UInt` | Replaces an array's elements in every configuration. Values are read as attribute values are, so `@string/name` is a reference. |
| `setStringArray(name: String, values: List<String>): UInt` | Replaces an array's elements with literal text, including numbers and text that starts with `@`. |
| `getArrayValues(name: String): List<ResourceValue>` | An array's elements without conversion, so references, dimensions, and colors survive a round trip. |
| `setArrayValues(name: String, values: List<ResourceValue>): UInt` | Replaces an existing array's elements without conversion. |

**The string pool**

The resource table stores all its text in one string pool, and string resources point into it by index. These functions work on the pool directly, for example to change one string everywhere it is used.

| Member | Description |
|---|---|
| `poolGet(index: UInt): String?` | The pool's string at `index`. |
| `poolSet(index: UInt, value: String)` | Changes the pool's string at `index`, for every resource that uses it. |
| `poolAdd(value: String): UInt?` | Adds a string to the pool and returns its index. |
| `poolFindRefs(stringIndex: UInt): List<ResourceRef>` | Every resource whose value is the pool's string at `stringIndex`. |
| `replaceEntry(resId: UInt, newStringIndex: UInt)` | Points one string resource at another pool string, leaving the others that shared its string alone. |

### `StyleScope`

The receiver of `resources.style { }`.

`set(attr: String, value: String)` sets one `<item>`: `this["android:windowBackground"] = "@color/black"`. `android:` names a framework attribute, and a name without a prefix one the app declares. An attribute that doesn't exist fails the patch.

### `ResourceValue`

A resource value without conversion: `kind`, Android's type byte, and `data`, its raw bits. For a string, `data` is an index in this APK's string pool, so copy strings between APKs with `poolGet` and `poolAdd`.

### `ResourceRef`

What `poolFindRefs` returns: `resId`, the resource's id, and `keyName`, its name. It is in `app.reseam.patch.types`.

## Files

### `FileScope`

The `files` of a running patch: every file in the APK.

| Member | Description |
|---|---|
| `list(): List<String>` | The path of every file. |
| `read(path: String): ByteArray?` | A file's contents. |
| `stream(path: String): InputStream?` | A file's contents as a stream, for files too large to read whole. It reads the file as it was when it opened, changes made so far included. Null when the file doesn't exist. Close it before your block ends. |
| `write(path: String, data: ByteArray)` | Adds or replaces a file, compressed. |
| `write(path: String, data: InputStream)` | Adds or replaces a file, compressed, with everything `data` reads, without holding it all in memory. When reading `data` fails, the file stays as it was. `data` stays open. |
| `writeStored(path: String, data: ByteArray)` | Adds or replaces a file, uncompressed. Android needs some files stored this way, such as `resources.arsc` and native libraries it maps. |
| `writeStored(path: String, data: InputStream)` | Adds or replaces a file, uncompressed, from a stream, as `write` does. |
| `delete(path: String)` | Removes a file. |
| `copy(bundlePath: String, apkPath: String)` | Copies a file from your bundle into the APK, compressed. Files you ship live in the project's `resources/` folder, so `bundlePath` is `resources/…`. A path that leads outside the bundle fails. |
| `xml(path: String): XmlDocument` | Opens a compiled XML file. Close it yourself, with `use { }`. |
| `editXml(path: String, block: XmlDocument.() -> T): T` | Opens the file, runs the block, and closes it. |
| `xmlFilesWith(vararg attributes: String): List<String>` | The compiled XML files where some element has one of these attributes, named as `XmlElement.get` takes them. It doesn't open every file to find them. |
| `sourceStream(): InputStream` | The original APK's bytes, streamed, including its signing block. Changes made so far don't show. Close it before your block ends. |
| `signers(): List<ByteArray>` | The original APK's signing certificates, DER-encoded X.509. Empty when the APK is unsigned or only has a v1 signature. |
| `components(): List<String>` | The names of every APK in the app. |
| `component(name: String): FileScope` | The files of another APK. |

## XML

Compiled XML files, the manifest included, open as an `XmlDocument`. Your changes are written back when you close the document, which `edit`, `editXml`, and `use { }` do for you.

A document's elements are handles into it. Take the values you need out before the document closes, never the elements themselves.

### `XmlDocument`

| Member | Description |
|---|---|
| `root: XmlElement` | The top element. |
| `findByTag(tag: String): List<XmlElement>` | Every element with this tag, in document order. |
| `findByAttribute(name: String, value: String): List<XmlElement>` | Every element whose attribute `name` is `value`. |
| `createElement(tag: String): XmlElement` | A new element, not yet attached. Attach it with `appendChild` or `insertBefore`. |
| `declareNamespace(prefix: String, uri: String)` | Declares a namespace prefix, unless the document already has it. |
| `adopt(element: XmlElement): XmlElement` | A copy of an element from another document, ready to attach here. Every attribute is matched to the resource id this document resolves it by, and one with no id fails the patch. Declare any namespace the source uses first. |
| `close()` | Writes your changes back and releases the document. `use { }` calls it. |
| `XmlDocument.compile(text: String): XmlDocument` | Compiles XML text into a document of its own, resolving `@type/name` references and attributes against the app's resources. It isn't a file in the APK: closing it discards it. Use it as a source for `adopt`. |

### `XmlElement`

| Member | Description |
|---|---|
| `tag: String` | The element's tag. |
| `parent: XmlElement?` | Its parent, or null for the root. |
| `children: List<XmlElement>` | Its child elements. |
| `get(attr: String): String?` | An attribute's value, written `element["android:label"]`. |
| `set(attr: String, value: String)` | Sets an attribute, written `element["android:label"] = "Settings"`. A prefixed name is bound to its resource id: `android:` through Android's own attributes, any other prefix through the app's, which the document must declare as a namespace. A name with no id fails the patch, since Android would ignore it. `value` is read as resource XML reads it, including enum and flag names such as `center` or `top\|start`. |
| `setInt(attr: String, value: Int)` | Sets an attribute to a number. |
| `setBool(attr: String, value: Boolean)` | Sets an attribute to a boolean. |
| `setResourceRef(attr: String, resId: UInt)` | Sets an attribute to a resource reference. |
| `removeAttribute(name: String)` | Removes an attribute. |
| `appendChild(child: XmlElement)` | Attaches an element as the last child. |
| `insertBefore(child: XmlElement, before: XmlElement): XmlElement` | Moves `child` in front of `before`, one of this element's children. Returns the moved element. |
| `remove()` | Removes this element and its children. |
| `clone(deep: Boolean = true): XmlElement` | A copy of this element, with its children unless `deep` is false. |

`appendChild` and `insertBefore` take elements of their own document. Use `adopt` to bring one over from another. An element you create or adopt is used up once attached. A handle to an element that no longer exists fails the patch.
