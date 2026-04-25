// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

fun BytecodeScope.classesExtending(superDescriptor: String): List<DexClass> =
    classes.filter { cls ->
        cls.superclass == superDescriptor ||
            runCatching { cls.superclassChain }.getOrNull().orEmpty().any { it.info.descriptor == superDescriptor }
    }

fun BytecodeScope.replaceAllStringsIndexed(old: String, new: String): Int {
    val methods = findInstructionsByString(old)
        .map { Method(it.method) }
        .distinctBy { method ->
            "${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
        }
    return methods.sumOf { it.replaceAllStrings(old, new) }
}
