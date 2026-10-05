package app.reseam.patch

import app.reseam.patch.native.hermesFindFunction
import app.reseam.patch.native.hermesVersion
import app.reseam.patch.native.hermesWrap

/**
 * The base APK's Hermes bytecode, opened on first use and written when the patch run finishes. The
 * default bundle is `assets/index.android.bundle`. Missing bundles, non-Hermes files and
 * unsupported bytecode versions fail the patch. Version 98 is currently supported.
 *
 * Functions and extensions are linked at patch time. Extension initialization runs once before the
 * app's global code, with exports held in a private lexical environment.
 */
class HermesScope internal constructor(private val path: String = "assets/index.android.bundle") {
    /** Opens the bundle and returns its bytecode version. */
    val version: Int
        get() = hermesVersion(path).toInt()

    /**
     * Uses another path in the base APK. A patch run can open one Hermes bundle; opening a
     * different path after the first bundle has been used fails the patch.
     */
    fun bundle(path: String): HermesScope = HermesScope(path)

    /**
     * Finds exactly one app function matching every constraint in [query]. Missing and ambiguous
     * matches fail with the query and match count. Extension functions are excluded.
     *
     * ```kotlin
     * hermes.function {
     *     name("canUseAnimatedEmojis")
     *     strings("ANIMATED_EMOJIS")
     *     paramCount(1)
     * }.wrap(Emotes.allow)
     * ```
     */
    fun function(query: HermesFunctionQuery.() -> Unit): HermesFunction {
        val match = HermesQuery().apply(query)
        return HermesFunction(hermesFindFunction(path, match.name, match.strings, match.parameters))
    }
}

/** Constraints combined with AND when finding a Hermes function. */
interface HermesFunctionQuery {
    /** Matches the function's bytecode name exactly, including case. */
    fun name(value: String)

    /** Requires every string to be referenced by an instruction, including property names. */
    fun strings(vararg values: String)

    /** Matches the declared JavaScript parameter count, excluding `this`. Must be nonnegative. */
    fun paramCount(count: Int)
}

private class HermesQuery : HermesFunctionQuery {
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
}

/** A stable function identity, valid only during the patch run that found it. */
class HermesFunction internal constructor(private val handle: UInt) {
    /**
     * Calls [export] as `export(original, ...arguments)` with the function's receiver, returning
     * the export's result. `original` invokes the unchanged body, bound to that receiver and closed
     * over the original outer environment. The export may call it with different arguments, skip it
     * or transform its result; exceptions propagate normally.
     *
     * The extension links once, when first used. Missing modules and undeclared or non-callable
     * exports fail the patch. Generators, async functions, class constructors, functions using
     * `new.target` or direct `eval`, repeated wrapping and unprovable environment chains are
     * refused. Ordinary wrapped functions cannot be called with `new` afterwards. Debug data for
     * edited functions may be dropped.
     */
    fun wrap(export: JsExport) = hermesWrap(handle, export.module.name, export.name)
}

/**
 * A JavaScript extension's artifact name, matching its workspace module: for
 * `apps/discord/extensions/emotes`, use `discord-emotes`. Sources live in `src/main/js` and assign
 * callable properties to `exports`. Keep export assignments unconditional.
 *
 * ```kotlin
 * object Emotes : ExtJsModule("discord-emotes") {
 *     val allow = export("allow")
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
