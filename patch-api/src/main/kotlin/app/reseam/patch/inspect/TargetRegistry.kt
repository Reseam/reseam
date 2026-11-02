// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.inspect

import app.reseam.patch.Target
import java.util.Collections
import java.util.WeakHashMap

/**
 * The targets created while a check of a bundle's targets runs, and whether a patch tried to
 * resolve each, so the check can name the targets it never reached. Targets are held weakly; a
 * bundle that is unloaded takes its targets with it. Outside a check, nothing is kept.
 */
internal object TargetRegistry {
    @Volatile private var checked: MutableMap<Target<*>, Boolean>? = null

    fun created(target: Target<*>) {
        checked?.put(target, false)
    }

    fun reached(target: Target<*>) {
        checked?.put(target, true)
    }

    fun start() {
        checked = Collections.synchronizedMap(WeakHashMap())
    }

    /** Ends the check: the labels of the targets it saw created that no patch tried to resolve. */
    fun finish(): List<String> {
        val targets = checkNotNull(checked) { "no target check is running" }
        checked = null
        return synchronized(targets) { targets.filterValues { !it }.keys.map { it.toString() } }
            .sorted()
    }
}
