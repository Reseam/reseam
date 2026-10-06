// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.CodeEmitter.Value

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
            Equality.EQUAL -> whenFalse(Value(result, Type.Int), block)
            Equality.NOT_EQUAL -> whenTrue(Value(result, Type.Int), block)
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
    this.block()
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
            this@branch.block()
            label(endLabel)
        }
    }
}
