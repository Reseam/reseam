---
title: "API: Patches"
description: Declaring patches, options, and what a patch sees while it runs.
---

# API: Patches

Everything here is in `app.reseam.patch`. The other API pages cover [targets and points](reference_targets.md), [code blocks](reference_code.md), [the manifest, resources, and files](reference_files.md), [settings](reference_settings.md), [Hermes](reference_hermes.md), [raw bytecode](reference_dex.md), the [app runtime](reference_runtime.md) your extensions call, and the [Gradle plugin](reference_gradle.md).

Wherever a parameter is a type written as a `String`, you can pass a descriptor (`Ljava/lang/String;`), a dotted name (`java.lang.String`), or a [`Type`](#type) constant.

## Declaring a patch

### `patch`

| Function | Description |
|---|---|
| `patch(name: StringRef, block: PatchBuilder.() -> Unit): ReseamPatch` | A patch users see, under `name`, a string from the module's string table. The name doesn't have to be unique. See [Names in every language](4_patches.md#names-in-every-language). |
| `patch(block: PatchBuilder.() -> Unit): ReseamPatch` | An internal patch. Users never see it, and it only runs when a patch that runs depends on it. |

Assign the result to a public top-level `val`. The engine finds patches by their declarations, and the patch's ID is the declaration's `package.property`.

### `PatchDeclaration`

What every patch declaration can say: which apps it is for and what it needs. [`PatchBuilder`](#patchbuilder) and the [settings host builder](reference_settings.md#settingshostbuilder) extend it.

| Member | Description |
|---|---|
| `compatibleWith(vararg packageNames: String)` | The patch is for these packages, any version. |
| `compatibleWith(vararg packages: CompatiblePackage)` | The patch is for these packages, limited to the versions each one lists. |
| `dependsOn(vararg patches: ReseamPatch)` | These patches run first. If one of them is skipped or fails, this patch is skipped. |

A patch with no `compatibleWith` works on any app. See [Which apps](4_patches.md#which-apps).

### `PatchBuilder`

The receiver of `patch { }`.

| Member | Description |
|---|---|
| `description(text: StringRef)` | The text users see under the name, from the module's string table. |
| `enabledByDefault(value: Boolean)` | Whether the patch starts selected. A patch for specific apps defaults to `true`, a patch for any app to `false`. |
| `hidden()` | Keeps a named patch off the lists users see. It still runs when selected or needed as a dependency. |
| `stringOption(key, title: StringRef, description: StringRef? = null, default: String? = null, validValues: List<String>? = null, required = false): StringOption` | Declares a text option. With `validValues`, the user picks one of them. |
| `boolOption(key, title: StringRef, description: StringRef? = null, default: Boolean? = null, required = false): BoolOption` | Declares an on/off option. |
| `intOption(key, title: StringRef, description: StringRef? = null, default: Long? = null, required = false): IntOption` | Declares a whole-number option. |
| `floatOption(key, title: StringRef, description: StringRef? = null, default: Double? = null, required = false): FloatOption` | Declares a decimal option. |
| `stringListOption(key, title: StringRef, description: StringRef? = null, default: List<String>? = null, required = false): StringListOption` | Declares a list-of-text option. |
| `pathOption(key, title: StringRef, description: StringRef? = null, required = false): PathOption` | Declares a folder the user picks. It has no default. |
| `settings(host: SettingsHost, vararg sections: SettingsSection)` | Adds these sections to the app's settings screen. `host` becomes a dependency. |
| `settings(host: SettingsHost, sections: PatchRuntime.() -> List<SettingsSection>)` | The same, with sections computed after `execute`, so they can depend on options. |
| `execute(block: PatchRuntime.() -> Unit)` | The change itself. It runs once all dependencies have run. |
| `afterDependents(block: PatchRuntime.() -> Unit)` | Runs after every patch that depends on this one has finished. |
| `afterWrite(block: OutputRuntime.() -> Unit)` | Runs on the written APK files, before they are signed. |

[When each block runs](4_patches.md#when-each-block-runs) explains the three blocks.

### `ReseamPatch`

What `patch` returns, and what the engine reads. You use the value to name a dependency or a settings host. You don't implement this interface.

| Member | Description |
|---|---|
| `name: StringRef?` | The display name, or null for an internal patch. |
| `hidden: Boolean` | Whether the patch is kept off the lists. |
| `description: StringRef?` | The description, or null for none. |
| `dependencies: List<ReseamPatch>` | The patches it depends on. |
| `compatibleWith: List<CompatiblePackage>` | The packages it is for. Empty for a patch that works on any app. |
| `universal: Boolean` | True when the patch declares no package. |
| `enabled: Boolean` | Whether it starts selected. |
| `options: List<Option<*>>` | Its options, in declaration order. |
| `execute(ctx: PatchRuntime)` | Runs the `execute` block. |
| `afterDependents(ctx: PatchRuntime)` | Runs the `afterDependents` block. |
| `finalizes: Boolean` | Whether the patch has an `afterDependents` block. The engine only calls it when this is set. |
| `afterWrite(ctx: OutputRuntime)` | Runs the `afterWrite` block. |
| `rewritesOutput: Boolean` | Whether the patch has an `afterWrite` block. Mount builds and the browser patcher leave such patches out. |

### `CompatiblePackage`

One app a patch is for, optionally limited to some versions.

| Member | Description |
|---|---|
| `CompatiblePackage(name: String, versions: List<String> = emptyList(), versionCodes: List<Int> = emptyList())` | An APK matches when its version name is in `versions` or its version code is in `versionCodes`. With both empty, every version matches. |
| `name: String` | The package name. |
| `versions: List<String>` | Version names. |
| `versionCodes: List<Int>` | `android:versionCode` values. |

Two shortcuts build one from a package name:

| Function | Description |
|---|---|
| `String.invoke(vararg versions: String): CompatiblePackage` | `"com.example.app"("2.14.0", "2.14.1")`: these version names. |
| `String.invoke(versionName: String, versionCode: Int): CompatiblePackage` | `"com.example.app"("2.14.0", 214001)`: one version, matched by its name or its code. |

### `ExternalPatch`

A patch from another bundle, for `dependsOn`. The Gradle plugin generates one value per patch of every bundle your module declares with [`reseam { bundle(...) }`](reference_gradle.md#reseam). You never construct one yourself.

| Member | Description |
|---|---|
| `bundle: String` | The other bundle's name. |
| `id: String` | The patch's ID in that bundle. |

The rest of its members are those of [`ReseamPatch`](#reseampatch). The engine runs the real patch from the other bundle, which users must also load.

## Options

### `Option`

The base of every option type. `options[option]` reads its value.

| Member | Description |
|---|---|
| `key: String` | The key users set it by, for example `--option "Clone app.packageName=..."` on the CLI. |
| `title: StringRef` | The label users see. |
| `description: StringRef?` | Help text users see, or null for none. |
| `required: Boolean` | Whether patching refuses to start without a value. |
| `kind: OptionKind` | The value's type. |
| `default: T?` | The value when the user sets none. |
| `validValues: List<String>?` | The values a text option allows, or null for any. |

The subclasses fix the value type: `StringOption` (`String`), `BoolOption` (`Boolean`), `IntOption` (`Long`), `FloatOption` (`Double`), `StringListOption` (`List<String>`), and `PathOption` (`OptionPath`). The `PatchBuilder` functions above create them.

### `StringOption`

A text option, read as `String`.

### `BoolOption`

An on/off option, read as `Boolean`.

### `IntOption`

A whole-number option, read as `Long`.

### `FloatOption`

A decimal option, read as `Double`.

### `StringListOption`

A list-of-text option, read as `List<String>`. On the CLI, repeat `--option` once per item.

### `PathOption`

A folder the user picks, read as [`OptionPath`](#optionpath).

### `OptionKind`

`STRING`, `BOOL`, `INT`, `FLOAT`, `STRING_LIST`, or `PATH`.

### `OptionPath`

The folder a user picked for a `PathOption`.

| Member | Description |
|---|---|
| `path: String` | The folder's path on the device or computer doing the patching. |
| `listContents(): List<String>` | The names of the files in the folder, sorted. |
| `readFile(relativePath: String): ByteArray?` | A file inside the folder. A path that leads outside the folder fails. |

### `RuntimeOptions`

The `options` of a running patch.

| Member | Description |
|---|---|
| `get(option: Option<T>): T` | `options[option]`: the value, with the default filled in. Fails when the option is optional, has no default, and the user set nothing. |
| `getOrNull(option: Option<T>): T?` | The value, or null when there is none. |

Before patching starts, the engine checks each value against its option: the type, the allowed values of a text option, and that a path exists. A patch only ever sees valid values.

## While a patch runs

### `PatchRuntime`

The receiver of `execute { }` and `afterDependents { }`.

| Member | Description |
|---|---|
| `manifest: ManifestScope` | The [manifest](reference_files.md#manifestscope). |
| `resources: ResourceScope` | The [resources](reference_files.md#resourcescope). |
| `files: FileScope` | Any [file in the APK](reference_files.md#filescope). |
| `bytecode: BytecodeScope` | Every [class](reference_code.md#bytecodescope), for lookups and app-wide changes. |
| `hermes: HermesScope` | The app's [Hermes bundle](reference_hermes.md#hermesscope). |
| `options: RuntimeOptions` | This patch's option values. |
| `log: PatchLogger` | Messages in the patch log. |
| `explain(target: Target<*>): MatchReport` | Why a target resolved the way it did. |

### `OutputRuntime`

The receiver of `afterWrite { }`. The patched app has been written by then, so only the files, the bundle, and the options are left.

| Member | Description |
|---|---|
| `output: OutputFiles` | The written base APK. |
| `options: RuntimeOptions` | This patch's option values. |
| `log: PatchLogger` | Messages in the patch log. |
| `bundleFile(path: String): File` | A file shipped in your bundle, at the path [`files.copy`](reference_files.md#filescope) takes. |
| `tempFile(name: String): File` | A path for a new file, in a folder the engine deletes after the run. It is on the same disk as the output, so `output.replace` can move it. |

### `OutputFiles`

One written APK, unsigned.

| Member | Description |
|---|---|
| `file: File` | The APK file. Read it, but don't write to it. Use `replace` instead. |
| `replace(replacement: File)` | Moves `replacement` over this APK. The engine signs whatever file is in place at the end. |
| `components(): List<String>` | The names of every written APK, the same names `files.components()` lists. |
| `component(name: String): OutputFiles` | Another written APK, such as a split. |

### `PatchLogger`

| Member | Description |
|---|---|
| `info(message: String)` | A message users see in the patch log. |
| `warn(message: String)` | A warning in the patch log. |
| `debug(message: String)` | A message shown only when debug logging is on. |

### `RunList`

Values that the patches of one run collect for another patch to use. A typical use is hooks that dependents register and a shared patch emits in its `afterDependents`.

Use a `RunList` instead of a top-level mutable list. A plain list outlives the run, and keeps values from a patch that failed and was undone. A `RunList` starts empty on every run, and drops the values of an undone patch.

```kotlin
private val hooks = RunList<ExtMethod>()

fun registerHook(hook: ExtMethod) = hooks.add(hook)

val hookOwner = patch {
    afterDependents { hooks.values.forEach { hook -> dispatch.before { call(hook) } } }
}
```

| Member | Description |
|---|---|
| `RunList<T>()` | An empty list. Declare it as a top-level value. |
| `add(value: T)` | Adds a value for the rest of the run. Only works while a patch is running. |
| `values: List<T>` | What the run has collected so far, in the order it was added. |

## Targets and reports

### `Target`

The base of every target: something a patch looks for. A target is searched for the first time a patch uses it, and the result is kept for the rest of that patch. The next patch searches again.

| Member | Description |
|---|---|
| `debugName: String?` | The label used in errors and reports. |
| `explain(): MatchReport` | Why the target resolved as it did. Only works while a patch is running. |

[API: Targets and points](reference_targets.md) lists the kinds of target.

### `MatchReport`

| Member | Description |
|---|---|
| `name: String` | The target's label. |
| `winner: String` | The method, class, or field it picked. |
| `considered: Int` | How many candidates the search looked at. |
| `reasons: List<String>` | What the query matched on. |
| `nearMisses: List<String>` | The closest candidates that didn't match, each with the reason. |

## Types

### `Type`

Descriptors for the types patches name most: `Void`, `Boolean`, `Byte`, `Short`, `Char`, `Int`, `Long`, `Float`, `Double`, `Object`, `String`, `CharSequence`, `List`, `ArrayList`, `Map`, `Context`, `View`, `Activity`, and `Application`.

### Type helpers

| Function | Description |
|---|---|
| `descriptor(type: String): String` | Any accepted form of a type as a descriptor: `descriptor("java.lang.String")` is `"Ljava/lang/String;"`. |
| `className(descriptor: String): String` | A class descriptor as a dotted name. |
| `proto(returns: String, vararg params: String): String` | A method prototype: `proto(Type.Boolean, Type.String)` is `"(Ljava/lang/String;)Z"`. |
