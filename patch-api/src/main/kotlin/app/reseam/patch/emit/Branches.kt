// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.CodeScope
import app.reseam.patch.Otherwise
import app.reseam.patch.Type
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType

internal enum class Equality {
    EQUAL,
    NOT_EQUAL,
}

internal fun CodeEmitter.equality(
    left: Value,
    right: Value,
    condition: Equality,
    block: CodeScope.() -> Unit,
): Otherwise {
    val comparison = comparisonOpcode(left.assignableType, right.assignableType)
    if (comparison != null) {
        val result = temporary(Type.Int)
        emit(template { reg3(comparison, 0, 0, 0) }, result, left, right)
        return when (condition) {
            Equality.EQUAL -> code.whenFalse(result, block)
            Equality.NOT_EQUAL -> code.whenTrue(result, block)
        }
    }
    return branch(block) { otherwise ->
        val skip =
            when (condition) {
                Equality.EQUAL -> template { ifNe(0, 0, 0) }
                Equality.NOT_EQUAL -> template { ifEq(0, 0, 0) }
            }
        emit(skip, left, right, target = otherwise)
    }
}

/** Runs [block] when [condition], given the label to branch to otherwise, falls through. */
internal fun CodeEmitter.branch(
    block: CodeScope.() -> Unit,
    condition: (otherwise: Label) -> Unit,
): Otherwise {
    val otherwise = label()
    val end = label()
    condition(otherwise)
    code.block()
    val otherwiseStep = steps.size
    place(otherwise)
    return object : Otherwise {
        override fun otherwise(block: CodeScope.() -> Unit) {
            // A then-block that already returned needs no jump over the else-block; the dead
            // goto would aim past the end of a replaced body, which the verifier rejects.
            if (steps[otherwiseStep - 1].fallsThrough)
                emitAt(otherwiseStep, template { goto(0) }, target = end)
            this@branch.code.block()
            place(end)
        }
    }
}

internal fun CodeEmitter.goto(label: Label) = emit(template { goto(0) }, target = label)

internal fun CodeEmitter.ifZero(value: Value, label: Label) {
    requireZeroComparable(value.assignableType)
    emit(template { ifEqz(0, 0) }, value, target = label)
}

internal fun CodeEmitter.ifNonZero(value: Value, label: Label) {
    requireZeroComparable(value.assignableType)
    emit(template { ifNez(0, 0) }, value, target = label)
}

/** Returns [value] from the method, or nothing when it is null. */
internal fun CodeEmitter.returnFromMethod(value: Value?) {
    val type = info.returnType
    val location = "return from ${info.descriptor}"
    if (value == null) {
        requireAssignableType(ValueType.Known(Type.Void), type, location)
        emit(template { returnVoid() })
        return
    }
    requireAssignableType(value.assignableType, type, location)
    require(type != Type.Void) { "${info.descriptor} returns no value" }
    val returned =
        when {
            isReferenceType(type) -> template { returnObject(0) }
            registerWordCount(type) == 2 -> template { returnWide(0) }
            else -> template { returnValue(0) }
        }
    emit(returned, value)
}
