---
description: What common errors mean, and how to fix them.
---

# Troubleshooting

Errors grouped by when you see them. [Testing and debugging](9_testing.md) covers the loop for finding them.

## While building

**`Reseam CLI not found`**
The build can't find `reseam`. Put it on your `PATH`, or set `RESEAM_BIN` to its path.

**`Unresolved reference 'before'`, or another hook**
Hooks such as `before`, `after`, `replace`, and `point` are extension functions, so each needs its import: `import app.reseam.patch.before`.

**The build fails while packing, naming a patch**
Packing loads every patch once to record its metadata. Top-level code that touches the app, such as resolving a target or reading `options`, fails here. Keep that code inside `execute { }`.

**A setting has no title**
A setting's title is the string named after its property in snake case, such as `hide_ads` for `hideAds`. Add it to `strings.xml`.

**A translation is refused**
A translation must keep the default string's arguments, with the same ids and types, and can't translate a string the default lacks.

**Your patch is missing from `reseam bundle list`**
Only public top-level values are found. Check that it isn't `private`, or declared inside an `object`.

## While patching

**`No method matched '<name>'`**
No method fits the target. The report lists what each constraint matched, and the closest misses with the reason each was rejected. See [When a patch fails](9_testing.md#when-a-patch-fails).

**`N methods matched '<name>'`**
The target is too loose. Add a constraint, rank the candidates with `rankBy`, or use `first()` if they are interchangeable.

**`This API is only available while a patch is executing`**
Something used a target, `options`, or a scope outside `execute { }`, for example in a top-level value.

**`ValueRef belongs to a different code block`**
A value was created in one code block and used in another. Create it again in the block that uses it, or pass it through a [reserved local](7_changing_code.md#passing-a-value-between-blocks).

**`… is an instance method; call it on a value`**
`call(method)` only calls static methods. Call an instance method on its object: `value.call(method)`.

**`the method body was replaced under it`**
A point was used after its method's body was replaced. Use the point before replacing the body, or don't replace it.

**`… needs a free register at or below v…`**
The code you added needs more registers than the instructions around it can reach. Move the logic into an [extension](10_extensions.md) and call it.

**`… is not in the app or any extension`**, or **`… is not defined by the app or any extension in the bundle`**
An extension class or method a patch refers to doesn't exist. The extension module may not be in the bundle, or the export was renamed. `reseam bundle list <bundle> --verbose` shows the bundle's DEX files.

**`OutOfMemoryError` while reading or writing a file**
`files.read` and `files.write` with a `ByteArray` hold the whole file in memory. Use `files.stream` and pass an `InputStream` to `files.write`, as [Large files](8_app_files.md#large-files) shows.

**`… stream is closed`**, or **`… stream belongs to another callback`**
A stream from `files.stream` or `files.sourceStream()` was read after it was closed, or kept past the block that opened it. Open it again where you read it.

**`the patched app can call extension methods that no applied patch implemented: …`**
An extension declares an `external` function that the patched app can reach, and no applied patch gives it a body. Enable the patch that implements it, or make the patch whose code leads there depend on that patch. See [Let a patch fill in a function](10_extensions.md#let-a-patch-fill-in-a-function).

**`Option '<key>' has no value`**
The option is optional, has no default, and the user set nothing. Read it with `options.getOrNull`, give it a default, or make it required.

**A patch is skipped**
The log says why: the app or its version doesn't match, or a dependency was skipped or failed. Patches nobody selected aren't part of the run, and don't appear at all.

**`1 patch(es) failed: …`, and no APK**
A run with a failed patch writes nothing by default. Fix the patch, or add `--leave-out-failed` to write the APK without it.

## Loading the bundle

**The signer is not trusted**
Pass the bundle's public key with `--trust` on the CLI. In Reseam Manager and the browser patcher, approve the signer when asked.

**The bundle needs a newer engine, or is too old**
Bundles only load on engines of their own release line. Rebuild the bundle with the plugin version that matches the engine, or update Reseam.

## In the patched app

**`NoClassDefFoundError` or `NoSuchMethodError`**
Patched code calls a class or method that isn't there. Check that the extension is in the bundle, and that your stubs match the real app. An app class your extension implements, such as the app's `kotlin.jvm.functions.Function0`, must be stubbed so it keeps the app's name.

**`VerifyError`, or `Rejecting class` in the log**
Android refused the patched code. Code blocks shouldn't produce this, so [report it](https://git.reseam.app/reseam/reseam/issues) with the method and the patch. In [raw bytecode](13_bytecode.md), check the registers and types you used.

**`NullPointerException` in an extension**
The app passed `null` where your function expects a value. Declare parameters nullable when the app can pass `null`.

**The patch applied, but nothing changed**
See [When the patch applied but nothing changed](9_testing.md#when-the-patch-applied-but-nothing-changed).

**The update won't install over the patched app**
It was signed with a different key. Android only accepts an update signed with the same key, so keep and back up the key you patch with.
