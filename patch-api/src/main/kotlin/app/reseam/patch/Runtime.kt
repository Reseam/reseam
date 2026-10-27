// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.checkInvocation
import app.reseam.patch.native.runningBundle
import app.reseam.patch.settings.SettingsHost
import app.reseam.patch.settings.SettingsSection
import app.reseam.patch.types.RunningBundleInfo
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
    val hermes: HermesScope = HermesScope()
    val files: FileScope = FileScope()
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()

    /** The bundle whose patch is running. */
    internal val bundle: RunningBundleInfo by lazy { runningBundle() }

    private val resolutions = IdentityHashMap<Target<*>, Resolution<*>>()
    private val resolving = IdentityHashMap<Target<*>, Unit>()
    private val successful = mutableListOf<() -> Unit>()
    internal val index: SearchIndex
        get() = run.cache.index

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
}

internal class PatchRun {
    private val journal = RunJournal()
    val cache = RunCache()
    val appModules = AppModules(journal)
    private val settings =
        IdentityHashMap<SettingsHost, MutableList<Pair<ReseamPatch, List<SettingsSection>>>>()
    private val settingsOwners = mutableMapOf<String, SettingsHost>()
    private val lists = IdentityHashMap<RunList<*>, MutableList<Any?>>()

    fun add(list: RunList<*>, value: Any?) {
        val values = lists.getOrPut(list) { mutableListOf() }
        values += value
        journal.record { values.removeAt(values.lastIndex) }
    }

    @Suppress("UNCHECKED_CAST")
    fun <T> values(list: RunList<T>): List<T> = lists[list].orEmpty().toList() as List<T>

    fun register(host: SettingsHost, patch: ReseamPatch, sections: List<SettingsSection>) {
        keepSettings(host)
        val registered = settings.getOrPut(host) { mutableListOf() }
        val index = registered.indexOfFirst { it.first === patch }
        if (index < 0) registered += patch to sections else registered[index] = patch to sections
    }

    fun sections(host: SettingsHost): List<SettingsSection> =
        settings[host].orEmpty().flatMap { it.second }

    fun clear(host: SettingsHost) {
        keepSettings(host)
        settings.remove(host)
    }

    /** Records that [host] writes [bundle]'s settings, which one host per app may do. */
    fun claimSettings(bundle: String, host: SettingsHost) {
        val owner = settingsOwners.putIfAbsent(bundle, host)
        require(owner == null || owner === host) {
            "$bundle has two settings hosts for this app: $owner and $host"
        }
        if (owner == null) journal.record { settingsOwners.remove(bundle) }
    }

    private fun keepSettings(host: SettingsHost) {
        val previous = settings[host]?.toList()
        journal.record {
            if (previous == null) settings.remove(host)
            else settings[host] = previous.toMutableList()
        }
    }

    @JvmName("openSavepoint") internal fun openSavepoint(savepoint: Int) = journal.open(savepoint)

    @JvmName("releaseSavepoint")
    internal fun releaseSavepoint(savepoint: Int) = journal.release(savepoint)

    @JvmName("rollbackTo") internal fun rollbackTo(savepoint: Int) = journal.rollback(savepoint)
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

internal class Resolution<R : Any>(val value: R, val report: MatchReport) {
    companion object {
        /** A value named outright rather than searched for. */
        fun <R : Any> direct(value: R, name: String, winner: String) =
            Resolution(
                value,
                SearchMatchReport(name, winner, 1, listOf("resolved directly"), emptyList()),
            )
    }
}

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
