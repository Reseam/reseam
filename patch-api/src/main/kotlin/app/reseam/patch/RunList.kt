// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.run.ActiveRuntime

/**
 * Values the patches of one run collect for another patch to use, such as the hooks dependents
 * register for a finalizer to emit. Each run starts with none, and a patch the engine takes back
 * takes its values with it. Declare one as a top-level value in place of a mutable collection,
 * which outlives the run and keeps what a taken-back patch added.
 *
 * ```kotlin
 * private val hooks = RunList<ExtMethod>()
 *
 * fun registerHook(hook: ExtMethod) = hooks.add(hook)
 *
 * val hookOwner = patch {
 *     afterDependents { hooks.values.forEach { hook -> emit(hook) } }
 * }
 * ```
 */
class RunList<T> {
    /** Adds [value] for the rest of the run. */
    fun add(value: T) = ActiveRuntime.current.run.add(this, value)

    /** What the run has collected, in the order it was added. */
    val values: List<T>
        get() = ActiveRuntime.current.run.values(this)
}
