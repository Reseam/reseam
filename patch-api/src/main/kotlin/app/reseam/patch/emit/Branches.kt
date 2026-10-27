// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.CodeScope
import app.reseam.patch.Otherwise
import app.reseam.patch.Type
import app.reseam.patch.descriptor
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
        val a = left.asByte().register
        val c = right.asByte().register
        val result = allocTemp()
        op(reads = listOf(a, c), writes = listOf(result)) { b, r ->
            b.reg3(
                comparison,
                byte(r(result), "cmp result"),
                byte(r(a), "cmp A"),
                byte(r(c), "cmp B"),
            )
        }
        return when (condition) {
            Equality.EQUAL -> code.whenFalse(Value(this, result, Type.Int), block)
            Equality.NOT_EQUAL -> code.whenTrue(Value(this, result, Type.Int), block)
        }
    }
    val a = left.asLow().register
    val c = right.asLow().register
    return branch(block) { elseLabel ->
        op(reads = listOf(a, c), target = elseLabel) { b, r ->
            when (condition) {
                Equality.EQUAL -> b.ifNe(low(r(a), "if-ne A"), low(r(c), "if-ne B"), elseLabel.name)
                Equality.NOT_EQUAL ->
                    b.ifEq(low(r(a), "if-eq A"), low(r(c), "if-eq B"), elseLabel.name)
            }
        }
    }
}

internal fun CodeEmitter.branch(
    block: CodeScope.() -> Unit,
    condition: (elseLabel: EmissionLabel) -> Unit,
): Otherwise {
    val elseLabel = nextLabel()
    val endLabel = nextLabel()
    condition(elseLabel)
    code.block()
    val elseIndex = ops.size
    label(elseLabel)
    return object : Otherwise {
        override fun otherwise(block: CodeScope.() -> Unit) {
            // A then-block that already returned needs no jump over the else-block; the dead
            // goto would aim past the end of a replaced body, which the verifier rejects.
            if (ops[elseIndex - 1].fallsThrough) {
                ops.add(
                    elseIndex,
                    Op(target = endLabel, fallsThrough = false) { b, _ ->
                        b.goto(endLabel.name)
                    },
                )
            }
            this@branch.code.block()
            label(endLabel)
        }
    }
}

internal fun CodeEmitter.label(name: EmissionLabel) {
    lowCopies.clear()
    op(label = name) { b, _ -> b.label(name.name) }
}

internal fun CodeEmitter.goto(label: EmissionLabel) {
    op(target = label, fallsThrough = false) { b, _ -> b.goto(label.name) }
}

internal fun CodeEmitter.ifZero(value: Value, label: EmissionLabel) {
    requireZeroComparable(value.assignableType)
    val v = value.asByte()
    op(reads = listOf(v.register), target = label) { b, r ->
        b.ifEqz(byte(r(v.register), "if-eqz"), label.name)
    }
}

internal fun CodeEmitter.ifNonZero(value: Value, label: EmissionLabel) {
    requireZeroComparable(value.assignableType)
    val v = value.asByte()
    op(reads = listOf(v.register), target = label) { b, r ->
        b.ifNez(byte(r(v.register), "if-nez"), label.name)
    }
}

internal fun CodeEmitter.nextLabel(): EmissionLabel = EmissionLabel(labelCounter++)

/** Returns [value] from the method, or nothing when it is null. */
internal fun CodeEmitter.returnFromMethod(value: Value?) {
    val type = info.returnType
    val location = "return from ${info.descriptor}"
    if (value == null) {
        requireAssignableType(ValueType.Known(Type.Void), type, location)
        op(fallsThrough = false) { b, _ -> b.returnVoid() }
        return
    }
    requireAssignableType(value.assignableType, type, location)
    require(type != Type.Void) { "${info.descriptor} returns no value" }
    val returned = value.asByte()
    op(reads = listOf(returned.register), fallsThrough = false) { b, r ->
        val register = byte(r(returned.register), "return")
        when {
            isReferenceType(type) -> b.returnObject(register)
            registerWordCount(type) == 2 -> b.returnWide(register)
            else -> b.returnValue(register)
        }
    }
}
