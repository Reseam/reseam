---
description: What common errors mean and how to fix them.
---

# Troubleshooting

## While building

**`Reseam CLI not found`**
The build can't find `reseam`. Put it on your `PATH`, or set `RESEAM_BIN` to its path.

**The build fails while packing, naming a patch**
Packing loads every patch once to record its metadata. Top-level code that touches the app (resolving a target, reading `options`) fails here. Keep that code inside `execute { }`.

**Your patch is missing from `reseam bundle list`**
Only public top-level values are found. Check it isn't `private` or inside an `object`.

## While patching

**`This API is only available while a patch is executing`**
Something used a target, `options`, or a scope outside `execute { }`.

**`No method matched '<name>'`**
No method fits the target. The report lists the closest misses and why each was rejected. See [Debugging a target](6_finding_code.md#debugging-a-target).

**`N methods matched '<name>'`**
The target is too loose. Add a constraint, rank the candidates with `rankBy`, or use `first()` if they are interchangeable.

**`ValueRef belongs to a different code block`**
A value was created in one code block and used in another. Recreate it in the block that uses it, or pass it through a [reserved local](8_changing_code.md#passing-a-value-between-blocks).

**`the method body was replaced under it`**
A point was used after its method's body was replaced. Use the point before replacing, or don't replace the body.

**`Cannot allocate ... registers`**
The code you added needs more registers than the instructions around it can address. Move the logic into an [extension](11_extensions.md) and call it.

**`... is not defined by the app or any extension in the bundle`**
An `ExtClass` name has a typo, or the extension module isn't in the bundle. `reseam bundle list --verbose` shows the bundle's DEX files.

**A patch is skipped**
The log says why: the app or version doesn't match, the user didn't select it, or a dependency was skipped or failed.

## Loading the bundle

**The signer is not trusted**
Pass the bundle's public key with `--trust` on the CLI. In Reseam Manager and the browser patcher, approve the signer when asked.

**The bundle needs a newer engine, or is too old**
Bundles load on engines of the same release line. Rebuild the bundle with the plugin version that matches the engine, or update Reseam.

## In the patched app

**`NoClassDefFoundError` or `NoSuchMethodError`**
Patched code calls a class or method that isn't there. Check the `ExtClass` declaration against your Java code, and stubs against the real app.

**The update won't install over the patched app**
It was signed with a different key. Android only accepts updates signed with the same key, so keep and back up your signing key.
