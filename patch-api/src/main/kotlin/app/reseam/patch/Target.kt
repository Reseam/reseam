// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.inspect.TargetInspection
import app.reseam.patch.inspect.TargetRegistry
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.Resolution

/**
 * Something a patch looks for: resolved against the running patch's APK the first time it is used,
 * cached for the rest of the patch.
 */
abstract class Target<R : Any>(val debugName: String?) {
    init {
        TargetRegistry.created(this)
    }

    internal abstract fun resolve(runtime: PatchRuntime): Resolution<R>

    /**
     * What the target matches in the running APK. A target found by query lists every candidate;
     * any other resolves, and its failure is the inspection's.
     */
    internal open fun inspect(runtime: PatchRuntime): TargetInspection =
        try {
            inspection(runtime.resolve(this))
        } catch (error: Exception) {
            TargetInspection.failed(error)
        }

    /** What [resolution], a resolution of this target, matched. */
    internal open fun inspection(resolution: Resolution<R>): TargetInspection =
        TargetInspection.of(resolution.report, listOf(resolution.report.winner), many = false)

    internal val resolved: R
        get() = ActiveRuntime.current.resolve(this).value

    internal open val label: String
        get() = debugName ?: "anonymous ${this::class.simpleName}"

    fun explain(): MatchReport = ActiveRuntime.current.resolve(this).report

    override fun toString(): String = label
}

/** Why a target resolved as it did: what won, how many candidates were searched, and why. */
class MatchReport
internal constructor(
    val name: String,
    val winner: String,
    val considered: Int,
    val reasons: List<String>,
    val nearMisses: List<String>,
)
