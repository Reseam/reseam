// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.PatchRuntime

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
