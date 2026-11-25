---
description: Find and edit a function in a native ARM library the app compiled its logic into.
---

# Native code

Some apps compile part of their logic into a native shared library (`lib/<abi>/*.so`) instead of DEX or Hermes bytecode. Reseam finds a function in such a library by its exported symbol, the strings its code references and the bytes of its code, and edits it: a constant return, a branch that always or never jumps, or raw bytes.

Code for `arm64-v8a` and `armeabi-v7a` can be edited. A query runs on each copy of the library the patch edits. When you patch for a phone, that is the copy the phone loads. When you patch for any phone, it is every `arm64-v8a` and `armeabi-v7a` copy.

A patch that edits native code fails when you patch for a phone that loads another ABI, such as `x86_64`. Copies for other ABIs stay as they are, and the patch log warns about each one.

## Find and edit

Declare the function as a target, then edit it in `execute`:

```kotlin
import app.reseam.patch.alwaysReturn
import app.reseam.patch.nativeFunction
import app.reseam.patch.patch

val forceAccess = patch(ExamplePatchStrings.forceAccess) {
    compatibleWith("com.example.app")
    execute {
        accessCheck.alwaysReturn(true)
    }
}

private val accessCheck = nativeFunction {
    library("libclient.so")
    referencesString("2026-11-app-access")
    single()
}
```

`library` is the file name of the `.so`. `referencesString` keeps the functions whose code loads the address of that string, resolved by following the address arithmetic to the data it points at. The string is matched as bytes, so it also matches a longer string that starts with it: `referencesString("feature-level")` matches code that loads `"feature-level-bonus"`. Pass the whole string, or add a second `referencesString` to narrow the match. `single()` requires the query to match exactly one function.

`alwaysReturn(true)` rewrites the function to `return 1`. Use it for a function that returns a boolean or int in the first result register.

## Match when several functions share a string

A string can be referenced by more than one function. `single()` then fails with the count, so order the matches by start address and take one:

```kotlin
private val accessCheck = nativeFunction {
    library("libclient.so")
    referencesString("2026-11-app-access")
    nth(0)
}
```

Each copy of the library is searched on its own, so `nth` counts the matches in each. Compilers usually lay out functions in the same order for every ABI, but not always.

Confirm the one you took is the function you meant. With [debug logging](9_testing.md#reading-the-log) on, the log names the address of the match in each library. Compare it with your disassembly, or check the effect on a device.

## Find a function by its bytes

A function that exports no symbol and references no unique string can be found by a run of its machine code. Copy the bytes from a disassembly, and write `??` for bytes that change between builds, such as the offsets in calls and address loads:

```kotlin
import app.reseam.patch.NativeAbi
import app.reseam.patch.nativeFunction

private val sync = nativeFunction("sync usage") {
    library("libclient.so")
    bytes(NativeAbi.ARM64_V8A, "fe 0f 1c f8 f8 5f 01 a9 ?? ?? ?? 94")
    bytes(NativeAbi.ARMEABI_V7A, "f0 b5 03 af ?? f0 ?? ??")
}
```

Machine code differs between ABIs, so a pattern only constrains the copy for its ABI. Give each copy the patch edits its own pattern, or another constraint such as `symbol`. A match must lie wholly inside one function. Patterns combine with `symbol` and `referencesString`, and every constraint must hold.

## Skip or force a branch

To change one decision inside a function, find its conditional branch by the bytes around it and make it always or never jump:

```kotlin
import app.reseam.patch.NativeAbi
import app.reseam.patch.branch
import app.reseam.patch.invoke
import app.reseam.patch.nativeFunction
import app.reseam.patch.neverTake
import app.reseam.patch.patch

val keepSyncing = patch(ExamplePatchStrings.keepSyncing) {
    compatibleWith("com.example.app"("4.2.0"))
    execute {
        sync.branch {
            bytes(NativeAbi.ARM64_V8A, "0a 2a 89 00 00 34")
            bytes(NativeAbi.ARMEABI_V7A, "?? 2c 18 b1")
        }.neverTake()
    }
}

private val sync = nativeFunction {
    library("libclient.so")
    symbol("sync_usage")
}
```

`branch` searches the function in each copy for the pattern of that copy's ABI. The matched bytes must cover exactly one conditional branch. A pattern may start in the instruction before the branch, to make it unique. `neverTake()` replaces the branch with a `nop` of the same size, so the code always continues with the next instruction. `alwaysTake()` replaces it with a jump to the same target. Reseam writes the instruction for the copy's instruction set, AArch64, ARM or Thumb, so you don't encode it.

The conditional branches are `b.cond`, `bc.cond`, `cbz`, `cbnz`, `tbz` and `tbnz` on `arm64-v8a`, and `b<cond>`, plus `cbz` and `cbnz` in Thumb code, on `armeabi-v7a`. A Thumb branch made conditional by a preceding `it` instruction is not one of them. Edit it with `replaceBytes`.

## Write raw instructions

For any other edit, write the bytes yourself at an offset inside the function. Take the offset and the instruction encoding from a disassembly of the function.

Instructions differ between ABIs, so give each ABI its own target with `abi` and its own bytes. This replaces `cmp w0, #2` with `cmp w0, #1` in the `arm64-v8a` copy, and `cmp r0, #2` with `cmp r0, #1` in the `armeabi-v7a` one:

```kotlin
import app.reseam.patch.NativeAbi
import app.reseam.patch.invoke
import app.reseam.patch.nativeFunction
import app.reseam.patch.patch
import app.reseam.patch.replaceBytes

val relaxAccess = patch(ExamplePatchStrings.relaxAccess) {
    compatibleWith("com.example.app"("4.2.0"))
    execute {
        accessCheck64.replaceBytes(0x18u, byteArrayOf(0x1f, 0x04, 0x00, 0x71))
        accessCheck32.replaceBytes(0xcu, byteArrayOf(0x01, 0x28))
    }
}

private val accessCheck64 = nativeFunction {
    library("libclient.so")
    abi(NativeAbi.ARM64_V8A)
    referencesString("2026-11-app-access")
}

private val accessCheck32 = nativeFunction {
    library("libclient.so")
    abi(NativeAbi.ARMEABI_V7A)
    referencesString("2026-11-app-access")
}
```

A target narrowed with `abi` matches nothing when the patch edits no copy for that ABI, for example when you patch for an `arm64-v8a` phone. Its edits then do nothing. `replaceBytes` fails on a target whose copies use different instruction sets, so a target without `abi` can only use it in an app that ships one ABI.

An `armeabi-v7a` function is either ARM code, with 4-byte instructions, or Thumb code, with 2- and 4-byte ones. Your disassembler shows which. Offsets count from the function's first instruction.

An edit reaching past the function's end fails.

## Things to know

- `alwaysReturn` sets the first result register, `w0` on `arm64-v8a` and `r0` on `armeabi-v7a`. It fits a function that returns a boolean or a 32-bit int. Check the function's return type before using it:
    - On `arm64-v8a`, a result larger than 16 bytes, such as a struct or a Rust `Result` around one, is returned in memory. The caller passes its address in `x8` and never reads `w0`.
    - On `armeabi-v7a`, a 64-bit result comes back in `r0` and `r1`, and a struct larger than 4 bytes is returned in memory whose address the caller passes in `r0`.
- An `arm64-v8a` library built with branch protection starts its functions with a landing pad, `bti` or `paciasp`. `alwaysReturn` keeps it, and checks the return address `paciasp` signed before it returns. Don't overwrite the pad with `replaceBytes`: an indirect call into the function then faults on devices that enforce it, and a `paciasp` removed from the start no longer matches the check before the function returns.
- A query is resolved against the library in the app you patch, and a new app version can move a symbol or string to other code. With `single()`, a changed match count fails the patch, so it can stay unpinned. With `nth()` or `replaceBytes`, a new build can make the same query hit other code without an error, so [pin](4_patches.md#which-apps) the versions you checked. Byte patterns describe one build's code, and a new build usually changes it. Pin the versions you checked for those too.
- A [mount build](8_app_files.md#mount-builds) leaves out a patch that edits native code when the app extracts its native libraries at install, because the installed app loads those copies. An app extracts them unless its manifest sets `android:extractNativeLibs="false"`.
- A function found only by `referencesString` has no symbol in a stripped library. Give it a `debugName` so failures name it.
- An `armeabi-v7a` library doesn't record where each function starts, because its unwind table merges neighbouring functions that unwind alike. Reseam finds the starts by following calls and branches from the functions the library does record. Hand-written assembly that jumps into the middle of another function can make that function look like two, so check the logged address of a match in such code.

Next: [Bundle projects](16_bundles.md).
