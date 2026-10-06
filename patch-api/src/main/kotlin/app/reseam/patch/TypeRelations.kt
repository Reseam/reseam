// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.isReferenceType

private val integralTypes = setOf("Z", "B", "S", "C", "I")

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

internal fun requireAssignableType(actual: String?, expected: String, location: String) {
    require(isAssignableType(actual, expected)) {
        "$location: ${actual ?: "null"} is incompatible with $expected"
    }
}

internal sealed interface ValueType {
    data class Known(val descriptor: String) : ValueType

    data object Zero : ValueType

    data object Null : ValueType
}

internal fun requireAssignableType(actual: ValueType, expected: String, location: String) {
    val compatible =
        when (actual) {
            is ValueType.Known -> isAssignableType(actual.descriptor, expected)
            ValueType.Zero -> expected in integralTypes || isAssignableType(null, expected)
            ValueType.Null -> isAssignableType(null, expected)
        }
    require(compatible) { "$location: $actual is incompatible with $expected" }
}

internal fun requireArgumentTypes(
    actual: List<ValueType>,
    expected: List<String>,
    location: String,
) {
    require(actual.size == expected.size) {
        "$location: passes ${actual.size} values, expected ${expected.size}"
    }
    actual.zip(expected).forEachIndexed { index, (value, destination) ->
        requireAssignableType(value, destination, "$location argument $index")
    }
}

internal fun requireZeroComparable(actual: ValueType) {
    require(
        actual !is ValueType.Known ||
            actual.descriptor in integralTypes ||
            isReferenceType(actual.descriptor)
    ) {
        "zero comparison requires an integral or reference value, got $actual"
    }
}

internal fun comparisonOpcode(left: ValueType, right: ValueType): Opcode? {
    if (
        left is ValueType.Known && right is ValueType.Known && left.descriptor == right.descriptor
    ) {
        when (left.descriptor) {
            Type.Long -> return Opcode.CMP_LONG
            Type.Float -> return Opcode.CMPL_FLOAT
            Type.Double -> return Opcode.CMPL_DOUBLE
        }
    }
    fun integral(value: ValueType): Boolean =
        value == ValueType.Zero || value is ValueType.Known && value.descriptor in integralTypes
    fun reference(value: ValueType): Boolean =
        value == ValueType.Null ||
            value == ValueType.Zero ||
            value is ValueType.Known && isReferenceType(value.descriptor)
    require(integral(left) && integral(right) || reference(left) && reference(right)) {
        "cannot compare $left with $right"
    }
    return null
}
