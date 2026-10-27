// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.hermesFindFunction
import app.reseam.patch.native.hermesHook
import app.reseam.patch.native.hermesModuleImports
import app.reseam.patch.native.hermesVersion
import app.reseam.patch.types.HermesHookKind
import app.reseam.patch.types.HermesImport
import app.reseam.patch.types.HermesModule

/**
 * The base APK's Hermes bytecode in `assets/index.android.bundle`, opened on first use and written
 * when the patch run finishes. Missing bundles, non-Hermes files and unsupported bytecode versions
 * fail the patch. Version 98 is currently supported.
 */
class HermesScope internal constructor() {
    /** Opens the bundle and returns its bytecode version. */
    val version: Int
        get() = hermesVersion().toInt()

    /**
     * Declares, for the rest of the run, the app modules JavaScript extensions import. An
     * extension's bare specifiers, such as `react` or `app:screens/Home.tsx`, name app modules;
     * each resolves to the Metro module declared here when the extension links. Patches whose
     * extensions import app modules depend on the patch that declares them.
     *
     * ```kotlin
     * val appModules = patch {
     *     execute {
     *         hermes.modules {
     *             module("react", function { name("isValidElement"); paramCount(1) }.module)
     *             prefix("app:") { path -> function { strings(path); paramCount(7) }.module }
     *         }
     *     }
     * }
     * ```
     */
    fun modules(block: AppModules.() -> Unit) = ActiveRuntime.current.run.appModules.block()
}

/** The app modules extensions import, by specifier, declared for the rest of the run. */
class AppModules internal constructor(private val journal: RunJournal) {
    private val modules = mutableMapOf<String, MetroModule>()
    private val prefixes = mutableMapOf<String, (String) -> MetroModule>()

    /** Resolves the import [specifier] to [module]. */
    fun module(specifier: String, module: MetroModule) {
        val previous = modules.putIfAbsent(specifier, module)
        require(previous == null || previous === module) { "$specifier is declared twice" }
        if (previous == null) journal.record { modules.remove(specifier) }
    }

    /**
     * Resolves imports that start with [prefix], and have no [module] declaration, by the rest of
     * the specifier.
     */
    fun prefix(prefix: String, module: (path: String) -> MetroModule) {
        val previous = prefixes.putIfAbsent(prefix, module)
        require(previous == null || previous === module) { "$prefix is declared twice" }
        if (previous == null) journal.record { prefixes.remove(prefix) }
    }

    internal fun resolve(specifier: String): MetroModule =
        modules.getOrPut(specifier) {
            val module =
                prefixes.entries
                    .firstOrNull { (prefix, _) -> specifier.startsWith(prefix) }
                    ?.let { (prefix, module) -> module(specifier.removePrefix(prefix)) }
                    ?: error(
                        "No app module is declared for the import $specifier; depend on the patch " +
                            "that declares the app's modules"
                    )
            journal.record { modules.remove(specifier) }
            module
        }
}

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

/** Replaces the function's body with `return null`. */
fun FunctionTarget.alwaysReturnNull() = replace { returnNull() }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: Boolean) = replace { returnValue(bool(value)) }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: Int) = replace { returnValue(int(value)) }

/** Replaces the function's body with `return value`. */
fun FunctionTarget.alwaysReturn(value: String) = replace { returnValue(string(value)) }

/**
 * A JavaScript extension of the bundle, named by its workspace module: for
 * `apps/example/extensions/features`, `example-features`. The Gradle plugin generates one binding
 * per extension from its entry module's exports; it is never written by hand.
 *
 * The extension's sources live in `src/main/js`: the entry module `index.ts`, `index.tsx`,
 * `index.js` or `index.jsx`, bundled with the modules it imports by relative path. Bare specifiers
 * name app modules, which [HermesScope.modules] declares; JSX uses the app's `react/jsx-runtime`.
 * The extension's code first runs when a hook first calls one of its exports.
 */
open class ExtJsModule(val name: String) {
    internal open val source: HermesModule
        get() =
            HermesModule.Bundle(
                name,
                hermesModuleImports(name).map { specifier ->
                    HermesImport(
                        specifier,
                        ActiveRuntime.current.run.appModules.resolve(specifier).handle,
                    )
                },
            )

    /** Declares a function the extension exports, named after the property, or [name]. */
    fun export(name: String? = null): MemberDelegate<JsExport> =
        MemberDelegate(name) { JsExport(this, it) }

    override fun toString() = name
}

/** A function an extension exports; hooks call it through [JsCodeScope.call]. */
class JsExport internal constructor(val module: ExtJsModule, val name: String) {
    override fun toString() = "$module.$name"
}
