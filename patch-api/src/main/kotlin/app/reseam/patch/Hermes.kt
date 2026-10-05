package app.reseam.patch

import app.reseam.patch.native.hermesFindFunction
import app.reseam.patch.native.hermesVersion
import app.reseam.patch.native.hermesWrap
import app.reseam.patch.types.HermesArgument

internal const val HERMES_BUNDLE = "assets/index.android.bundle"

/**
 * The base APK's Hermes bytecode in `assets/index.android.bundle`, opened on first use and written
 * when the patch run finishes. Missing bundles, non-Hermes files and unsupported bytecode versions
 * fail the patch. Version 98 is currently supported.
 *
 * Functions and extensions are linked at patch time. Extension initialization runs once before the
 * app's global code, with exports held in a private lexical environment.
 */
class HermesScope internal constructor() {
    /** Opens the bundle and returns its bytecode version. */
    val version: Int
        get() = hermesVersion(HERMES_BUNDLE).toInt()
}

/**
 * The one app function in the Hermes bundle matching every constraint in [block]. Missing and
 * ambiguous matches fail with the query and match count. Extension functions are excluded.
 *
 * ```kotlin
 * private val canUseAnimatedEmojis = function {
 *     name("canUseAnimatedEmojis")
 *     strings("ANIMATED_EMOJIS")
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
        Resolution(
            hermesFindFunction(HERMES_BUNDLE, query.name, query.strings, query.parameters),
            wrapped(label, query.toString()),
        )

    /**
     * Calls [export] as `export(original, ...arguments)` with the function's receiver, returning
     * the export's result. `original` invokes the previous wrap, or the unchanged body for the
     * first wrap, bound to that receiver and closed over the original outer environment. The export
     * may call it with different arguments, skip it or transform its result; exceptions propagate
     * normally.
     *
     * Wraps compose in application order (patch execution order), including exports from different
     * modules. The last wrap runs outermost; the innermost `original` calls the unchanged app body.
     * Lookups continue to match original app functions.
     *
     * The extension links once, when first used. Missing modules and undeclared or non-callable
     * exports fail the patch. Generators, async functions, class constructors, functions using
     * `new.target` or direct `eval`, and unprovable environment chains are refused. Ordinary
     * wrapped functions cannot be called with `new` afterwards. Debug data for edited functions may
     * be dropped.
     */
    fun wrap(export: JsExport) = wrap(export, emptyList())

    /** Like [wrap], passing [bound] to [export] ahead of `original`. */
    internal fun wrap(export: JsExport, bound: List<HermesArgument>) =
        hermesWrap(resolved, export.module.name, export.name, bound)
}

/**
 * A JavaScript extension's artifact name, matching its workspace module: for
 * `apps/discord/extensions/emotes`, use `discord-emotes`. Sources live in `src/main/js` and assign
 * callable properties to `exports`. Keep export assignments unconditional.
 *
 * ```kotlin
 * object Emotes : ExtJsModule("discord-emotes") {
 *     val canUseAnimatedEmojis = export("canUseAnimatedEmojis")
 * }
 * ```
 */
open class ExtJsModule(val name: String) {
    init {
        require(
            name.isNotEmpty() &&
                name.all { it.isLetterOrDigit() && it.code < 128 || it == '-' || it == '_' }
        ) {
            "invalid Hermes module name: $name"
        }
    }

    /** Declares an exact property name on the extension's exports object. Validated on use. */
    fun export(name: String): JsExport = JsExport(this, name)
}

/** A declared callable JavaScript export; it does not load or execute the module by itself. */
class JsExport internal constructor(val module: ExtJsModule, val name: String)
