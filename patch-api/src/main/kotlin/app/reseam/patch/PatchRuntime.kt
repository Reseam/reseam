// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.inspect.TargetInspection
import app.reseam.patch.inspect.TargetRegistry
import app.reseam.patch.native.checkInvocation
import app.reseam.patch.native.logDebug
import app.reseam.patch.native.logInfo
import app.reseam.patch.native.logWarn
import app.reseam.patch.native.reportResolution
import app.reseam.patch.native.runningBundle
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.MethodEdits
import app.reseam.patch.run.Resolution
import app.reseam.patch.search.SearchIndex
import app.reseam.patch.types.RunningBundleInfo
import java.util.IdentityHashMap

/**
 * What a patch sees while it runs. Constructed by the engine for each `execute` and
 * `afterDependents` call and passed as the receiver.
 */
class PatchRuntime internal constructor(internal val run: PatchRun) {
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
        val recording = run.recordsResolutions
        if (recording) TargetRegistry.reached(target)
        val resolution =
            try {
                target.resolve(this)
            } catch (error: Exception) {
                if (recording) {
                    val inspection = runCatching {
                        target.inspect(this).withFailure(error)
                    }
                        .getOrElse { TargetInspection.failed(error) }
                    report(target, inspection)
                }
                throw error
            } finally {
                resolving.remove(target)
            }
        resolutions[target] = resolution
        if (recording && resolution.searched) report(target, target.inspection(resolution))
        log.debug("${target.label}: ${resolution.report.winner}")
        return resolution
    }

    /** Tells the engine how [target] resolved, for hosts that check a bundle's targets. */
    private fun report(target: Target<*>, inspection: TargetInspection) {
        reportResolution(
            target.label,
            inspection.matches,
            inspection.winner,
            inspection.many,
            inspection.considered.toUInt(),
            inspection.reasons,
            inspection.nearMisses,
            inspection.failure,
        )
    }
}

class PatchLogger internal constructor() {
    fun info(message: String) = logInfo(message)

    fun warn(message: String) = logWarn(message)

    fun debug(message: String) = logDebug(message)
}
