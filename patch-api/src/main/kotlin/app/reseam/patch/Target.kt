// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.Resolution

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

/** Why a target resolved as it did: what won, how many candidates were searched, and why. */
class MatchReport
internal constructor(
    val name: String,
    val winner: String,
    val considered: Int,
    val reasons: List<String>,
    val nearMisses: List<String>,
)
