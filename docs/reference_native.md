---
title: "API: Native libraries"
description: Finding and editing functions in a native ARM library in the app.
---

# API: Native libraries

For logic the app compiles into a native ARM shared library (`lib/<abi>/*.so`) instead of DEX or Hermes bytecode. The guide page is [Native code](15_native.md). Everything here is in `app.reseam.patch`.

`arm64-v8a` and `armeabi-v7a` code can be edited. A query runs on each copy of the library the patch edits: the one the device loads, or every `arm64-v8a` and `armeabi-v7a` copy when patching for any device. Patching for a device that loads another ABI fails the patch. Copies for other ABIs stay unedited, with a warning in the patch log for each.

## Finding functions

### `nativeFunction`

`nativeFunction(debugName: String? = null, block: NativeFunctionQuery.() -> Unit): NativeFunctionTarget` is a function of a native library, found by its exported symbol, the strings its code references and byte patterns in its code. A query can match several functions, so pin it with `single()` or `nth()`.

### `NativeFunctionQuery`

| Member | Matches functions that |
|---|---|
| `library(name: String)` | are in the library with this file name, such as `libclient.so`. Required. A patch whose library has no `arm64-v8a` or `armeabi-v7a` copy the device loads fails. |
| `abi(abi: NativeAbi)` | are in the copy of the library for this ABI. Without it, every copy the patch edits is searched. A target with no copy for its ABI matches nothing, and its edits do nothing. |
| `symbol(name: String)` | are defined under this exported symbol. |
| `referencesString(vararg values: String)` | compute the address of each of these strings in loaded data. A string is matched as bytes, so a longer string that starts with it matches too. |
| `bytes(abi: NativeAbi, pattern: String)` | hold `pattern` in their code, in the copy for `abi`: hex bytes separated by spaces, with `??` for any byte, such as `"1f 20 ?? d5"`. The match must lie wholly inside the function. A pattern constrains only the copy for its ABI. Repeated patterns must all match. |
| `single()` | are the only match in each copy; several or none fail. The default. |
| `nth(index: Int)` | are the match at this index in start-address order in each copy, when the query matches several. |

### `NativeAbi`

An ABI whose native code can be edited, for `NativeFunctionQuery.abi` and the `bytes` of a query.

| Member | Description |
|---|---|
| `ARM64_V8A` | 64-bit ARM, with AArch64 code. |
| `ARMEABI_V7A` | 32-bit ARM, with ARM and Thumb functions. |

### `NativeFunctionTarget`

A function in a native library. The verbs below edit it.

| Function | Description |
|---|---|
| `NativeFunctionTarget.alwaysReturn(value: Boolean)` | Replaces the function's code with a return of 1 or 0 in the first result register, `w0` or `r0`. Valid only when the function returns a boolean or 32-bit int there. |
| `NativeFunctionTarget.alwaysReturn(value: Int)` | Replaces the function's code with `return value`, in the instruction set of each copy. On `arm64-v8a`, both forms keep a leading `bti` or `paciasp` landing pad, and check the return address `paciasp` signed before returning. |
| `NativeFunctionTarget.replaceBytes(offset: ULong, bytes: ByteArray)` | Writes `bytes` over the function's code at `offset` bytes from its start. Fails when the target's copies use different instruction sets, so narrow it with `abi`. An edit reaching past the function fails. |

## Branches

### `branch`

`NativeFunctionTarget.branch(debugName: String? = null, block: NativeBranchQuery.() -> Unit): NativeBranchTarget` is one conditional branch in the function, in each copy of the library the function was found in.

### `NativeBranchQuery`

| Member | Description |
|---|---|
| `bytes(abi: NativeAbi, pattern: String)` | The branch in the copy for `abi`: `pattern` must match in the function's code where its bytes cover exactly one conditional branch. Patterns are written as in `NativeFunctionQuery.bytes`. Each copy needs a pattern for its ABI, and one per ABI is allowed. |

The conditional branches are `b.cond`, `bc.cond`, `cbz`, `cbnz`, `tbz` and `tbnz` in AArch64 code, `b<cond>` in ARM code, and `b<cond>`, `cbz` and `cbnz` in Thumb code. A Thumb branch inside an `it` block is not one.

### `NativeBranchTarget`

A conditional branch in a native function. Both verbs write an instruction of the branch's size in its instruction set.

| Function | Description |
|---|---|
| `NativeBranchTarget.alwaysTake()` | Replaces the branch with an unconditional jump to the same target. |
| `NativeBranchTarget.neverTake()` | Replaces the branch with a `nop`, so the code continues with the next instruction. |
