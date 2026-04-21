# Runtime surface

`ctx` (type `PatchRuntime`) is passed to `execute` and `afterDependents`. It exposes five scopes and a logger:

| Scope | Access |
|-------|--------|
| `bytecode` | Classes, methods, instructions. [Fingerprint](3_2_fingerprints.md) lookups. |
| `manifest` | The Android manifest (AXML). |
| `resources` | Resource table and string pool. |
| `files` | Read, write, delete, copy files inside the APK. |
| `options` | Option values set by the caller. |
| `log` | `info(msg)`, `warn(msg)`, `debug(msg)`. |

`manifest`, `resources`, and `files` are split-APK aware: `.component(name)` narrows the scope to a named split (see [Split APKs](#split-apks) below).

## Bytecode

```kotlin
for (cls in ctx.bytecode.classes) { /* ... */ }

val target = ctx.bytecode.findClass("Lcom/example/app/SomeClass;")
    ?: error("class not found")

for (method in target.methods.filter { it.info.methodName == "doThing" }) {
    // ...
}
```

### Whole-method replacement

- `method.returnEarly()`, `returnEarly(value: Int|Boolean|Long)`, `returnEarlyNull()`. Replace the body with a constant return.
- `method.setInstructions(insns)`, `replaceBody(registersSize, outsSize, insns)`. Swap the body wholesale.

### In-place rewriting

- `method.replaceString(old, new)`, `replaceAllStrings(old, new)`. Rewrite `const-string` values.
- `method.replaceLiteral(old, new)`, `replaceAllLiterals(old, new)`.
- `method.replaceMethodCall(index, newClass, newName, newProto)`. Retarget an `invoke-*` at a known index.

### Targeted splices

- `method.insertInvokeStatic(index, className, name, proto, registers)`. Splice a static call.
- `method.insertInvokeStaticWithMoveResult(index, className, name, proto, registers, resultRegister, isObject)`. Same, capturing the return value.
- `method.addInstructions(index) { ... }`. Build and insert a sequence with the [instruction builder](3_3_instructions.md).

### Locating instructions

- `method.indexOfFirst(opcode)`, `indexOfFirstReversed(opcode, start)`.
- `method.indexOfFirstLiteral(value)`, `indexOfFirstString(s)`.
- `method.indexOfFirstMethodCall(definingClass, methodName, start)`.
- `method.indexOfFirstFieldAccess(opcode, fieldType?, definingClass?, start)`.
- `method.indexOfOpcodeSequence(opcodes, start)`.

A typical rewrite combines lookup and splice:

```kotlin
val idx = method.indexOfFirstMethodCall(
    "Lcom/example/Api;", "call",
) ?: error("call site not found")

method.replaceMethodCall(
    idx,
    "Lcom/example/PatchedApi;", "patchedCall", "(Ljava/lang/String;)V",
)
```

For matching methods whose names change across app versions, use [fingerprints](3_2_fingerprints.md).

## Manifest

```kotlin
ctx.manifest.addPermission("android.permission.INTERNET")
ctx.manifest.setVersionName("1.2.3")
ctx.manifest.setActivityConfigChanges(
    "com.example.app.MainActivity",
    "orientation|screenSize",
)
ctx.manifest.addIntentFilter(
    "com.example.app.MainActivity",
    action = "android.intent.action.VIEW",
)
```

Top-level helpers on `ctx.manifest`: `addPermission`, `setVersionCode`, `setVersionName`, `setMinSdk`, `setAttributeInt`, `setAttributeString`, `setActivityConfigChanges`, `addIntentFilter`, `addActivityAlias`, `copyIntentFilters`. Read-only: `packageName`, `versionCode`, `versionName`, `minSdkVersion`, `splitName`.

For anything beyond the helpers, open the document:

```kotlin
ctx.manifest.document().use { doc ->
    doc.findByTag("application").forEach { app ->
        val activity = doc.createElement("activity").apply {
            this["android:name"] = "com.example.app.addon.AddonActivity"
            this["android:exported"] = "true"
        }
        app.appendChild(activity)
    }
}
```

Lookup: `doc.findByTag(tag)`, `doc.findByAttribute(name, value)`. Read and write: `el["android:name"]`. Numeric and boolean attributes: `el.setInt`, `el.setBool`. Resource references: `el.setResourceRef("android:theme", resId)`. Tree editing: `appendChild`, `insertBefore`, `remove`, `clone(deep)`.

## Resources

```kotlin
ctx.resources.setString("app_name", "New App Name")
ctx.resources.addString("patch_message", "This app was patched")
ctx.resources.addColor("primary_color", "#FF0000")
ctx.resources.addInteger("max_connections", 10)

val poolIdx = ctx.resources.poolAdd("replacement_label")!!
ctx.resources.replaceEntry(ctx.resources.id("string", "app_name")!!, poolIdx)
```

- `id(resType, resName)`: resolve a resource ID. `exists(resType, resName)`: presence check.
- `getString(name)`, `setString(name, value)`, `addString(name, value)`. Same shape for `addBool`, `addInteger`, `addColor`, `addDimen`, `addId`, `addRaw`.
- `poolAdd(string)`, `poolGet(index)`, `poolSet(index, value)`, `poolFindRefs(index)`.
- `replaceEntry(resId, poolIndex)`: redirect a resource ID to a different string-pool entry.
- `owningComponent(resId)` / `owningComponent(resType, resName)`: locate which split a resource lives in.

## Files

```kotlin
val names = ctx.files.list()
val config = ctx.files.read("assets/config.json")
ctx.files.write("META-INF/patch.txt", "Patched!".toByteArray())
ctx.files.copy("bundle_asset.png", "res/drawable/patch_icon.png")
ctx.files.delete("unneeded_file.xml")
```

- `list()`: every file path in the APK.
- `read(path)`, `write(path, bytes)`, `delete(path)`, `copy(bundlePath, apkPath)`.
- `xml(path)` / `useXml(path) { ... }`: open a bundled AXML document.

`useXml` gives you the same `XmlDocument` API the manifest uses:

```kotlin
ctx.files.useXml("res/xml/config.xml") {
    findByTag("config").forEach { cfg ->
        cfg["enabled"] = "true"
        cfg.setInt("timeout", 30000)

        val setting = createElement("setting").apply {
            this["name"] = "patched"
        }
        cfg.appendChild(setting)
    }

    val added = createElement("config").apply {
        this["key"] = "patch_version"
        this["value"] = "1.0"
    }
    root.appendChild(added)
}
```

## Split APKs

Every `component(name)` method returns a narrowed scope targeting one split:

```kotlin
ctx.manifest.component("base").addPermission("android.permission.INTERNET")
ctx.manifest.component("config.arm64_v8a").setVersionCode(789)

ctx.resources.component("base").addString("app_name", "Base")
ctx.resources.component("config.xxhdpi").addBool("feature_flag", true)

ctx.files.component("base").write("assets/base_config.json", "{}".toByteArray())
ctx.files.component("feature_dynamic").copy("dynamic_asset.png", "res/drawable/dynamic.png")
```

`ctx.manifest.components()`, `ctx.resources.components()`, `ctx.files.components()` enumerate what's available. When no component is specified, operations run against the base component.
