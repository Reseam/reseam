// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.types.HermesBlock
import app.reseam.patch.types.HermesCode
import app.reseam.patch.types.HermesCondition
import app.reseam.patch.types.HermesHookKind
import app.reseam.patch.types.HermesOperand
import app.reseam.patch.types.HermesPlace
import app.reseam.patch.types.HermesStatement

/**
 * Code a patch adds to an app JavaScript function, the Hermes counterpart of [CodeScope]. Values
 * are [JsValue]s. Kotlin control flow runs while the patch is applied; `when*` blocks become
 * branches in the app, testing as JavaScript does: truthiness, `== null` and strict equality.
 */
interface JsCodeScope {
    /** The receiver the function was called with. */
    val thisObject: JsValue

    /** The argument at [index], as assigned so far; `undefined` past the last. */
    fun param(index: Int): JsValue

    /** The value the function returned, inside [after]. */
    val result: JsValue

    fun int(value: Int): JsValue

    fun bool(value: Boolean): JsValue

    fun string(value: String): JsValue

    val nullValue: JsValue

    val undefinedValue: JsValue

    /** Calls [export] with `this` undefined. */
    fun call(export: JsExport, vararg args: JsValue): JsValue

    operator fun JsExport.invoke(vararg args: JsValue): JsValue = call(this, *args)

    /** Runs [block] when [value] is truthy. */
    fun whenTrue(value: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    /** Runs [block] when [value] is falsy. */
    fun whenFalse(value: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    /** Runs [block] when [value] is `null` or `undefined`. */
    fun whenNull(value: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    /** Runs [block] when [value] is neither `null` nor `undefined`. */
    fun whenNotNull(value: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    /** Runs [block] when `left === right`. */
    fun whenEqual(left: JsValue, right: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    /** Runs [block] when `left !== right`. */
    fun whenNotEqual(left: JsValue, right: JsValue, block: JsCodeScope.() -> Unit): JsOtherwise

    fun returnValue(value: JsValue)

    fun returnUndefined()

    fun returnNull()

    fun returnTrue()

    fun returnFalse()
}

interface JsOtherwise {
    infix fun otherwise(block: JsCodeScope.() -> Unit)
}

interface JsValue {
    /**
     * Overwrites this value in place: a parameter, the result, or an earlier call's result. A
     * constant cannot be assigned.
     */
    fun assign(value: JsValue)
}

/** Builds a hook's code, block by block, as the patch's Kotlin runs. */
internal class JsCode(private val kind: HermesHookKind) {
    private val blocks = mutableListOf<MutableList<HermesStatement>>()
    private var current = 0
    private var slots = 0u
    private val scope = Scope()

    fun build(block: JsCodeScope.() -> Unit): HermesCode {
        current = open()
        scope.block()
        return HermesCode(blocks.map { HermesBlock(it.toList()) })
    }

    private fun open(): Int {
        blocks += mutableListOf<HermesStatement>()
        return blocks.lastIndex
    }

    private fun emit(statement: HermesStatement) {
        blocks[current] += statement
    }

    private fun operand(value: JsValue): HermesOperand {
        require(value is Value && value.code === this) { "$value belongs to another hook" }
        return value.operand
    }

    private inner class Value(val operand: HermesOperand, private val place: HermesPlace?) :
        JsValue {
        val code: JsCode
            get() = this@JsCode

        override fun assign(value: JsValue) {
            requireNotNull(place) { "$operand cannot be assigned" }
            emit(HermesStatement.Assign(place, operand(value)))
        }

        override fun toString() = operand.toString()
    }

    private inner class Scope : JsCodeScope {
        override val thisObject: JsValue = Value(HermesOperand.Receiver, null)

        override fun param(index: Int): JsValue {
            require(index >= 0) { "parameter index must be nonnegative" }
            return Value(HermesOperand.Param(index.toUInt()), HermesPlace.Param(index.toUInt()))
        }

        override val result: JsValue
            get() {
                require(kind == HermesHookKind.AFTER) { "result exists only in an after hook" }
                return Value(HermesOperand.Result, HermesPlace.Result)
            }

        override fun int(value: Int): JsValue = Value(HermesOperand.Int(value), null)

        override fun bool(value: Boolean): JsValue = Value(HermesOperand.Bool(value), null)

        override fun string(value: String): JsValue = Value(HermesOperand.Text(value), null)

        override val nullValue: JsValue = Value(HermesOperand.Null, null)

        override val undefinedValue: JsValue = Value(HermesOperand.Undefined, null)

        override fun call(export: JsExport, vararg args: JsValue): JsValue {
            val slot = slots++
            emit(
                HermesStatement.Call(
                    slot,
                    export.module.source,
                    export.name,
                    args.map { operand(it) },
                )
            )
            return Value(HermesOperand.Slot(slot), HermesPlace.Slot(slot))
        }

        override fun whenTrue(value: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.True(operand(value)), block)

        override fun whenFalse(value: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.False(operand(value)), block)

        override fun whenNull(value: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.Null(operand(value)), block)

        override fun whenNotNull(value: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.NotNull(operand(value)), block)

        override fun whenEqual(left: JsValue, right: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.Equal(operand(left), operand(right)), block)

        override fun whenNotEqual(left: JsValue, right: JsValue, block: JsCodeScope.() -> Unit) =
            branch(HermesCondition.NotEqual(operand(left), operand(right)), block)

        override fun returnValue(value: JsValue) = emit(HermesStatement.Return(operand(value)))

        override fun returnUndefined() = emit(HermesStatement.Return(HermesOperand.Undefined))

        override fun returnNull() = emit(HermesStatement.Return(HermesOperand.Null))

        override fun returnTrue() = emit(HermesStatement.Return(HermesOperand.Bool(true)))

        override fun returnFalse() = emit(HermesStatement.Return(HermesOperand.Bool(false)))

        private fun branch(condition: HermesCondition, block: JsCodeScope.() -> Unit): JsOtherwise {
            val (then, otherwise) = open() to open()
            emit(HermesStatement.Branch(condition, then.toUInt(), otherwise.toUInt()))
            inBlock(then, block)
            return object : JsOtherwise {
                override fun otherwise(block: JsCodeScope.() -> Unit) = inBlock(otherwise, block)
            }
        }

        private fun inBlock(block: Int, code: JsCodeScope.() -> Unit) {
            val outer = current
            current = block
            try {
                code()
            } finally {
                current = outer
            }
        }
    }
}
