// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.checkInvocation
import app.reseam.patch.native.getClassInfo
import app.reseam.patch.native.getClassInfos
import app.reseam.patch.native.getMethodInfo
import app.reseam.patch.native.getMethodInfos
import app.reseam.patch.native.mutationChanges
import app.reseam.patch.native.mutationRevision
import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.types.ClassInfo
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
    private val methodInfos = HashMap<UInt, MethodInfo>()
    private val classInfos = HashMap<UInt, ClassInfo>()
    private var revision = mutationRevision()
    private var searchIndex: SearchIndex? = null
    private val successful = mutableListOf<() -> Unit>()
    internal val index: SearchIndex
        get() {
            synchronize()
            return searchIndex ?: SearchIndex(this).also { searchIndex = it }
        }

    internal fun synchronize() {
        val current = mutationRevision()
        if (current == revision) return
        val changes = mutationChanges(revision)
        revision = current
        if (changes == null) {
            methodInfos.clear()
            classInfos.clear()
            searchIndex?.invalidate()
        } else {
            changes.forEach { methodInfos.remove(it) }
            searchIndex?.invalidateMethods(changes)
        }
    }

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

    private inline fun <T> synchronizedInfo(read: () -> T): T {
        synchronize()
        return read()
    }

    internal fun methodInfo(handle: UInt): MethodInfo = synchronizedInfo {
        methodInfos.getOrPut(handle) {
            getMethodInfo(handle) ?: error("invalid method handle: $handle")
        }
    }

    internal fun classInfo(handle: UInt): ClassInfo = synchronizedInfo {
        classInfos.getOrPut(handle) {
            getClassInfo(handle) ?: error("invalid class handle: $handle")
        }
    }

    internal fun prefetchMethods(handles: Collection<UInt>) {
        synchronize()
        handles.filterNot(methodInfos::containsKey).chunked(256).forEach { batch ->
            val infos = getMethodInfos(batch.toUIntArray())
            check(infos.size == batch.size) { "Invalid method in query candidates" }
            batch.zip(infos).forEach { (handle, info) -> methodInfos[handle] = info }
        }
    }

    internal fun prefetchClasses(handles: Collection<UInt>) {
        synchronize()
        handles.filterNot(classInfos::containsKey).chunked(256).forEach { batch ->
            val infos = getClassInfos(batch.toUIntArray())
            check(infos.size == batch.size) { "Invalid class in query candidates" }
            batch.zip(infos).forEach { (handle, info) -> classInfos[handle] = info }
        }
    }
}

internal class PatchRun {
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
