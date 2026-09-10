// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import java.util.IdentityHashMap

/**
 * What a patch sees while it runs. Constructed by the engine for each
 * `execute` and `afterDependents` call and passed as the receiver.
 */
class PatchRuntime {
    val manifest: ManifestScope = ManifestScope()
    val resources: ResourceScope = ResourceScope()
    val bytecode: BytecodeScope = BytecodeScope()
    val files: FileScope = FileScope()
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()

    private val resolutions = IdentityHashMap<Target<*>, Resolution<*>>()
    private val resolving = IdentityHashMap<Target<*>, Unit>()
    private val methodInfos = HashMap<UInt, MethodInfo>()
    private val classInfos = HashMap<UInt, ClassInfo>()
    internal val index: SearchIndex by lazy { SearchIndex(this) }

    /** Why a target resolved the way it did. */
    fun explain(target: Target<*>): MatchReport = resolve(target).report

    internal fun <R : Any> resolve(target: Target<R>): Resolution<R> {
        resolutions[target]?.let {
            @Suppress("UNCHECKED_CAST")
            return it as Resolution<R>
        }
        check(resolving.put(target, Unit) == null) {
            "${target.label} depends on itself through another target"
        }
        val resolution = try {
            target.resolve(this)
        } finally {
            resolving.remove(target)
        }
        resolutions[target] = resolution
        log.debug("${target.label}: ${resolution.report.winner}")
        return resolution
    }

    internal fun methodInfo(handle: UInt): MethodInfo =
        methodInfos.getOrPut(handle) { getMethodInfo(handle) ?: error("invalid method handle: $handle") }

    internal fun classInfo(handle: UInt): ClassInfo =
        classInfos.getOrPut(handle) { getClassInfo(handle) ?: error("invalid class handle: $handle") }
}

/** The runtime of the patch being executed, reached implicitly by targets and scopes. */
internal object ActiveRuntime {
    private var active: PatchRuntime? = null

    val current: PatchRuntime
        get() = active ?: error("This API is only available while a patch is executing.")

    fun <T> run(runtime: PatchRuntime, block: () -> T): T {
        val previous = active
        active = runtime
        try {
            return block()
        } finally {
            active = previous
        }
    }
}

interface MatchReport {
    val name: String
    val winner: String
    val considered: Int
    val reasons: List<String>
    val nearMisses: List<String>
}

internal data class SearchMatchReport(
    override val name: String,
    override val winner: String,
    override val considered: Int,
    override val reasons: List<String>,
    override val nearMisses: List<String>,
) : MatchReport

internal class Resolution<R : Any>(val value: R, val report: MatchReport)

/**
 * Something a patch looks for: resolved against the running patch's APK the
 * first time it is used, cached for the rest of the patch.
 */
abstract class Target<R : Any>(val debugName: String?) {
    internal abstract fun resolve(runtime: PatchRuntime): Resolution<R>

    internal val resolved: R
        get() = ActiveRuntime.current.resolve(this).value

    internal open val label: String
        get() = debugName ?: "anonymous ${this::class.simpleName}"

    fun explain(): MatchReport = ActiveRuntime.current.resolve(this).report

    override fun toString(): String = label
}

internal fun noMatchMessage(kind: String, report: MatchReport): String = buildString {
    append("No $kind matched '${report.name}'. Searched ${report.considered} candidate(s).")
    if (report.reasons.isNotEmpty()) append("\nReasons: ${report.reasons.joinToString("; ")}")
    if (report.nearMisses.isNotEmpty()) append("\nNear misses: ${report.nearMisses.joinToString("; ")}")
}
