// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.Type
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.native.isAssignable

internal val integralTypes = setOf("Z", "B", "S", "C", "I")

internal sealed interface ValueType {
    data class Known(val descriptor: String) : ValueType

    data object Zero : ValueType

    data object Null : ValueType
}

internal fun requireAssignableType(actual: ValueType, expected: String, location: String) {
    val compatible =
        when (actual) {
            is ValueType.Known -> isAssignable(actual.descriptor, expected)
            ValueType.Zero -> expected in integralTypes || isReferenceType(expected)
            ValueType.Null -> isReferenceType(expected)
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
