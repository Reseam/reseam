// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.Type
import app.reseam.patch.run.ActiveRuntime

internal val integralTypes = setOf("Z", "B", "S", "C", "I")

internal fun isAssignableType(actual: String?, expected: String): Boolean {
    if (actual == null) return expected.startsWith("L") || expected.startsWith("[")
    if (actual == expected) return true
    if (actual in integralTypes && expected in integralTypes) return true
    val reference = actual.startsWith("L") || actual.startsWith("[")
    if (!reference || !(expected.startsWith("L") || expected.startsWith("["))) return false
    if (expected == Type.Object) return true
    if (actual.startsWith("[")) {
        if (expected in setOf("Ljava/lang/Cloneable;", "Ljava/io/Serializable;")) return true
        if (!expected.startsWith("[")) return false
        val element = actual.drop(1)
        val destination = expected.drop(1)
        return if (element.startsWith("L") || element.startsWith("["))
            isAssignableType(element, destination)
        else element == destination
    }
    if (expected.startsWith("[")) return false
    fun hierarchy(root: String): Set<String>? {
        val pending = ArrayDeque<String>().apply { add(root) }
        val ancestors = mutableSetOf<String>()
        while (pending.isNotEmpty()) {
            val type = pending.removeFirst()
            if (!ancestors.add(type) || type == Type.Object) continue
            val clazz = ActiveRuntime.current.index.classFor(type) ?: return null
            clazz.superclass?.let(pending::add)
            pending.addAll(clazz.interfaces)
        }
        return ancestors
    }
    // Missing platform or library definitions cannot prove an invalid reference assignment.
    val ancestors = hierarchy(actual) ?: return true
    return expected in ancestors || hierarchy(expected) == null
}
