// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.hermesFindFunction
import app.reseam.patch.native.hermesHook
import app.reseam.patch.run.Resolution
import app.reseam.patch.types.HermesHookKind

/**
 * The one app function in the Hermes bundle matching every constraint in [block]. Missing and
 * ambiguous matches fail with the query and match count. Extension functions are excluded.
 *
 * ```kotlin
 * private val isFeatureEnabled = function {
 *     name("isFeatureEnabled")
 *     strings("FEATURE_FLAGS")
 *     paramCount(1)
 * }
 * ```
 */
fun function(debugName: String? = null, block: HermesFunctionQuery.() -> Unit): FunctionTarget =
    FunctionTarget(debugName, HermesQuery().apply(block))

/** Constraints combined with AND when finding a Hermes function. */
interface HermesFunctionQuery {
    /** Matches the function's bytecode name exactly, including case. */
    fun name(value: String)

    /** Requires every string to be referenced by an instruction, including property names. */
    fun strings(vararg values: String)

    /** Matches the declared JavaScript parameter count, excluding `this`. Must be nonnegative. */
    fun paramCount(count: Int)
}

internal class HermesQuery : HermesFunctionQuery {
    var name: String? = null
    val strings = mutableListOf<String>()
    var parameters: UInt? = null

    override fun name(value: String) {
        name = value
    }

    override fun strings(vararg values: String) {
        strings += values
    }

    override fun paramCount(count: Int) {
        require(count >= 0) { "parameter count must be nonnegative" }
        parameters = count.toUInt()
    }

    override fun toString() = buildList {
        name?.let { add("name $it") }
        if (strings.isNotEmpty()) add("strings ${strings.joinToString()}")
        parameters?.let { add("$it parameters") }
    }
        .joinToString(", ")
}

/** A function in the app's Hermes bundle. */
class FunctionTarget internal constructor(debugName: String?, private val query: HermesQuery) :
    Target<UInt>(debugName) {
    override val label: String
        get() = debugName ?: query.name ?: "function with $query"

    override fun resolve(runtime: PatchRuntime): Resolution<UInt> =
        Resolution.direct(
            hermesFindFunction(query.name, query.strings, query.parameters),
            label,
            query.toString(),
        )

    internal fun hook(kind: HermesHookKind, block: JsCodeScope.() -> Unit) =
        hermesHook(resolved, kind, JsCode(kind).build(block))

    /**
     * The Metro module that defines this function, for declaring app modules. React Native bundles
     * register each source module with Metro; its exports hold the module's functions. The module
     * is the one whose factory creates this function, or this function when it is a factory. A
     * function created outside a module, or in several, fails the patch.
     */
    val module: MetroModule
        get() = MetroModule(this)
}

/**
 * A Metro module of the app's bundle, named by a function it defines. See [FunctionTarget.module].
 */
class MetroModule internal constructor(private val function: FunctionTarget) {
    internal val handle: UInt
        get() = function.resolved
}

/**
 * Runs [block] when the function is entered. Falling off its end calls the function with its
 * arguments, as [block] assigned them; [JsCodeScope.returnValue] returns early instead.
 *
 * Hooks compose in application order (patch execution order): a later hook runs around the function
 * as earlier hooks left it. Lookups continue to match original app functions. Each extension links
 * once, when a hook first calls it; missing modules and undeclared exports fail the patch.
 * Generators, async functions, class constructors, functions using `new.target` or direct `eval`,
 * and unprovable environment chains are refused. Hooked functions cannot be called with `new`
 * afterwards. Debug data for edited functions may be dropped.
 */
fun FunctionTarget.before(block: JsCodeScope.() -> Unit) = hook(HermesHookKind.BEFORE, block)

/**
 * Runs [block] after the function returns; [JsCodeScope.result] is its result, which falling off
 * the end returns. See [before].
 */
fun FunctionTarget.after(block: JsCodeScope.() -> Unit) = hook(HermesHookKind.AFTER, block)

/**
 * Runs [block] instead of the function; falling off its end returns `undefined`. Earlier hooks no
 * longer run. See [before].
 */
fun FunctionTarget.replace(block: JsCodeScope.() -> Unit) = hook(HermesHookKind.REPLACE, block)

/** Replaces the function's body with `return undefined`. */
fun FunctionTarget.alwaysReturn() = replace { returnUndefined() }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: Boolean) = replace { returnValue(bool(value)) }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: Int) = replace { returnValue(int(value)) }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: String) = replace { returnValue(string(value)) }

/** Replaces the function's body with `return null`. */
fun FunctionTarget.alwaysReturnNull() = replace { returnNull() }
