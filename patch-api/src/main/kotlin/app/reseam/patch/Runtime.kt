// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.checkInvocation
import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.types.ClassInfo
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo
import java.util.IdentityHashMap

/**
 * What a patch sees while it runs. Constructed by the engine for each `execute` and
 * `afterDependents` call and passed as the receiver.
 */
class PatchRuntime() {
    internal var run = PatchRun()
        private set

    internal constructor(run: PatchRun) : this() {
        this.run = run
    }

    val manifest: ManifestScope = ManifestScope()
    val resources: ResourceScope = ResourceScope()
    val bytecode: BytecodeScope = BytecodeScope()
    val files: FileScope = FileScope()
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()

    private val resolutions = IdentityHashMap<Target<*>, Resolution<*>>()
    private val resolving = IdentityHashMap<Target<*>, Unit>()
    private val successful = mutableListOf<() -> Unit>()
    internal val index: SearchIndex
        get() = run.cache.index

    internal fun synchronize() = run.cache.synchronize()

    internal fun afterSuccess(action: () -> Unit) {
        successful += action
    }

    @JvmName("invokeExecute")
    internal fun invokeExecute(patch: ReseamPatch) = invoke { patch.execute(this) }

    @JvmName("invokeAfterDependents")
    internal fun invokeAfterDependents(patch: ReseamPatch) = invoke { patch.afterDependents(this) }

    private fun invoke(action: () -> Unit) =
        ActiveRuntime.run(this) {
            action()
            checkInvocation()
            successful.forEach { it() }
            successful.clear()
        }

    internal val edits: MethodEdits = MethodEdits()

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
        val resolution =
            try {
                target.resolve(this)
            } finally {
                resolving.remove(target)
            }
        resolutions[target] = resolution
        log.debug("${target.label}: ${resolution.report.winner}")
        return resolution
    }

    internal fun methodInfo(handle: UInt): MethodInfo = run.cache.methodInfo(handle)

    internal fun classInfo(handle: UInt): ClassInfo = run.cache.classInfo(handle)

    internal fun instructions(handle: UInt): List<Instruction> = run.cache.instructions(handle)

    internal fun prefetchMethods(handles: Collection<UInt>) = run.cache.prefetchMethods(handles)

    internal fun prefetchClasses(handles: Collection<UInt>) = run.cache.prefetchClasses(handles)
}

internal class PatchRun {
    val cache = RunCache()
    private val settings =
        IdentityHashMap<SettingsHost, MutableList<Pair<ReseamPatch, List<SettingsSection>>>>()

    fun register(host: SettingsHost, patch: ReseamPatch, sections: List<SettingsSection>) {
        val registered = settings.getOrPut(host) { mutableListOf() }
        val index = registered.indexOfFirst { it.first === patch }
        if (index < 0) registered += patch to sections else registered[index] = patch to sections
    }

    fun sections(host: SettingsHost): List<SettingsSection> =
        settings[host].orEmpty().flatMap { it.second }

    fun clear(host: SettingsHost) {
        settings.remove(host)
    }
}

internal object ActiveRuntime {
    private val active = ThreadLocal<PatchRuntime>()

    val current: PatchRuntime
        get() = active.get() ?: error("This API is only available while a patch is executing.")

    val currentOrNull: PatchRuntime?
        get() = active.get()

    fun <T> run(runtime: PatchRuntime, block: () -> T): T {
        val previous = active.get()
        active.set(runtime)
        try {
            return block()
        } finally {
            if (previous == null) active.remove() else active.set(previous)
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
 * Something a patch looks for: resolved against the running patch's APK the first time it is used,
 * cached for the rest of the patch.
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
    if (report.nearMisses.isNotEmpty())
        append("\nNear misses: ${report.nearMisses.joinToString("; ")}")
}
