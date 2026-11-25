// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.nativeAlwaysReturn
import app.reseam.patch.native.nativeAlwaysTake
import app.reseam.patch.native.nativeFindBranch
import app.reseam.patch.native.nativeFindFunction
import app.reseam.patch.native.nativeNeverTake
import app.reseam.patch.native.nativeReplaceBytes
import app.reseam.patch.run.Resolution
import app.reseam.patch.types.NativeBytes
import app.reseam.patch.types.NativeLibraryAbi

/**
 * A function of a native library in the app, found by its exported symbol, the strings its code
 * references and byte patterns in its code. The query runs on each copy of the library the patched
 * app keeps: `arm64-v8a` and `armeabi-v7a` code can be edited. Patching for a device that loads
 * another ABI fails, and patching for any device leaves the other ABIs' copies unedited.
 *
 * A query may match several functions, so pin it with [NativeFunctionQuery.single] or
 * [NativeFunctionQuery.nth] and confirm the match.
 *
 * ```kotlin
 * private val accessCheck = nativeFunction {
 *     library("libclient.so")
 *     referencesString("2026-11-app-access")
 *     single()
 * }
 * ```
 */
fun nativeFunction(
    debugName: String? = null,
    block: NativeFunctionQuery.() -> Unit,
): NativeFunctionTarget = NativeFunctionTarget(debugName, NativeQuery().apply(block))

/** An ABI whose native code can be edited. */
enum class NativeAbi(internal val bridge: NativeLibraryAbi) {
    /** 64-bit ARM, whose code is AArch64. */
    ARM64_V8A(NativeLibraryAbi.ARM64_V8A),

    /** 32-bit ARM, whose code mixes A32 and Thumb functions. */
    ARMEABI_V7A(NativeLibraryAbi.ARMEABI_V7A),
}

/** Constraints combined with AND when finding a native function. */
interface NativeFunctionQuery {
    /** The library's file name, such as `libclient.so`. Required. */
    fun library(name: String)

    /**
     * Matches only the copy of the library for [abi]. [replaceBytes] needs it, since raw bytes are
     * code of one instruction set. When the patched app keeps no copy for [abi], the target matches
     * nothing and its edits do nothing.
     */
    fun abi(abi: NativeAbi)

    /** Matches a function defined under this exported symbol. */
    fun symbol(name: String)

    /**
     * Requires the function's code to compute the address of each string in loaded data. A string
     * is matched as bytes, so a longer string that starts with it matches too.
     */
    fun referencesString(vararg values: String)

    /**
     * Requires the code of the function in the copy of the library for [abi] to hold [pattern]: hex
     * bytes separated by spaces, with `??` for any byte, such as `"fe 0f 1c f8 ?? 5f 01 a9"`.
     * Machine code differs by ABI, so a pattern constrains only the copy for its ABI; give each
     * copy its own. Repeated patterns must all match.
     */
    fun bytes(abi: NativeAbi, pattern: String)

    /** Requires the query to match exactly one function. The default when neither is set. */
    fun single()

    /** Takes the match at [index] in start-address order when the query matches several. */
    fun nth(index: Int)
}

internal class NativeQuery : NativeFunctionQuery {
    var library: String? = null
    var abi: NativeAbi? = null
    var symbol: String? = null
    val strings = mutableListOf<String>()
    val bytes = mutableListOf<NativeBytes>()
    var nth: UInt? = null

    override fun library(name: String) {
        library = name
    }

    override fun abi(abi: NativeAbi) {
        this.abi = abi
    }

    override fun symbol(name: String) {
        symbol = name
    }

    override fun referencesString(vararg values: String) {
        strings += values
    }

    override fun bytes(abi: NativeAbi, pattern: String) {
        bytes += NativeBytes(abi.bridge, pattern)
    }

    override fun single() {
        nth = null
    }

    override fun nth(index: Int) {
        require(index >= 0) { "match index must be nonnegative" }
        nth = index.toUInt()
    }

    fun libraryName(): String = requireNotNull(library) { "a native function needs a library()" }

    override fun toString() = buildList {
        abi?.let { add("abi $it") }
        symbol?.let { add("symbol $it") }
        if (strings.isNotEmpty()) add("references ${strings.joinToString()}")
        bytes.forEach { add(it.description()) }
        nth?.let { add("match $it") }
    }
        .joinToString(", ")
}

/** A function in a native library of the app. */
class NativeFunctionTarget
internal constructor(debugName: String?, private val query: NativeQuery) : Target<UInt>(debugName) {
    override val label: String
        get() = debugName ?: query.symbol ?: "native function with $query"

    override fun resolve(runtime: PatchRuntime): Resolution<UInt> =
        Resolution.direct(
            nativeFindFunction(
                query.libraryName(),
                query.abi?.bridge,
                query.symbol,
                query.strings,
                query.bytes,
                query.nth,
            ),
            label,
            "${query.libraryName()}: $query",
        )
}

/** Replaces the function's code with `return value` in the first result register. */
fun NativeFunctionTarget.alwaysReturn(value: Boolean) =
    nativeAlwaysReturn(resolved, if (value) 1 else 0)

/** Replaces the function's code with `return value`. */
fun NativeFunctionTarget.alwaysReturn(value: Int) = nativeAlwaysReturn(resolved, value)

/**
 * Writes [bytes] over the function's code at [offset] bytes from its start. The bytes are code of
 * one instruction set, so the patch fails when the target matched copies with different ones:
 * narrow it with [NativeFunctionQuery.abi].
 */
fun NativeFunctionTarget.replaceBytes(offset: ULong, bytes: ByteArray) =
    nativeReplaceBytes(resolved, offset, bytes)

/**
 * The conditional branch in the function that [block] finds, in each copy of the library the
 * function was found in. Its verbs rewrite it in the copy's instruction set.
 *
 * ```kotlin
 * private val sync = nativeFunction {
 *     library("libclient.so")
 *     symbol("sync_usage")
 * }
 * // in execute { }
 * sync.branch {
 *     bytes(NativeAbi.ARM64_V8A, "0a 2a 89 00 00 34 01")
 *     bytes(NativeAbi.ARMEABI_V7A, "?? 2c 18 b1")
 * }.neverTake()
 * ```
 */
fun NativeFunctionTarget.branch(
    debugName: String? = null,
    block: NativeBranchQuery.() -> Unit,
): NativeBranchTarget = NativeBranchTarget(debugName, this, BranchQuery().apply(block))

/** Finds one conditional branch in a native function by the code around it. */
interface NativeBranchQuery {
    /**
     * The branch in the copy of the library for [abi]: [pattern] must match in the function's code
     * where its bytes cover the branch and no other conditional branch. A pattern is written as in
     * [NativeFunctionQuery.bytes]. Each copy the function was found in needs a pattern for its ABI.
     */
    fun bytes(abi: NativeAbi, pattern: String)
}

internal class BranchQuery : NativeBranchQuery {
    val bytes = mutableListOf<NativeBytes>()

    override fun bytes(abi: NativeAbi, pattern: String) {
        require(bytes.none { it.abi == abi.bridge }) { "a branch has one pattern per ABI" }
        bytes += NativeBytes(abi.bridge, pattern)
    }

    override fun toString() = bytes.joinToString(", ") { it.description() }
}

/** A conditional branch in a function of a native library of the app. */
class NativeBranchTarget
internal constructor(
    debugName: String?,
    private val function: NativeFunctionTarget,
    private val query: BranchQuery,
) : Target<UInt>(debugName) {
    override val label: String
        get() = debugName ?: "branch in ${function.label}"

    override fun resolve(runtime: PatchRuntime): Resolution<UInt> =
        Resolution.direct(
            nativeFindBranch(runtime.resolve(function).value, query.bytes),
            label,
            "${function.label}: $query",
        )
}

/** Makes the branch jump whatever its condition. */
fun NativeBranchTarget.alwaysTake() = nativeAlwaysTake(resolved)

/** Makes the branch fall through whatever its condition. */
fun NativeBranchTarget.neverTake() = nativeNeverTake(resolved)

private fun NativeBytes.description() = "${abi.name.lowercase()} bytes $pattern"
