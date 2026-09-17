// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

/** Conservative assignability: app hierarchy plus the host's java.* platform classes. */
internal fun isAssignableType(actual: String, expected: String): Boolean {
    if (actual == expected) return true
    val reference = actual.startsWith("L") || actual.startsWith("[")
    if (!reference || !(expected.startsWith("L") || expected.startsWith("["))) return false
    if (expected == Type.Object) return true
    if (actual.startsWith("[")) {
        if (expected in setOf("Ljava/lang/Cloneable;", "Ljava/io/Serializable;")) return true
        return expected.startsWith("[") && isAssignableType(actual.drop(1), expected.drop(1))
    }
    val visited = mutableSetOf<String>()
    fun known(type: String): Boolean {
        if (type == expected) return true
        if (!visited.add(type)) return false
        val clazz = ActiveRuntime.current.index.classFor(type)
        if (clazz != null) return clazz.interfaces.any(::known) || clazz.superclass?.let(::known) == true
        // Android's java.util collection interfaces are shared with the JVM. Do not
        // load app/extension classes or assume relationships for missing Android types.
        if (!type.startsWith("Ljava/") || !expected.startsWith("Ljava/")) return false
        return runCatching {
            val loader = ClassLoader.getPlatformClassLoader()
            Class.forName(className(expected), false, loader)
                .isAssignableFrom(Class.forName(className(type), false, loader))
        }.getOrDefault(false)
    }
    return known(actual)
}
