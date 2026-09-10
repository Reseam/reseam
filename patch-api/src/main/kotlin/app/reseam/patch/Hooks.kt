// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.opcode

/** Runs `block` when the method is entered. */
fun MethodTarget.before(block: CodeScope.() -> Unit) = method.insertCode(0, emptyList(), block)

/** Runs `block` before every return; `capture("result")` is the value being returned. */
fun MethodTarget.after(block: CodeScope.() -> Unit) {
    val target = method
    val insns = target.instructions
    val returns = insns.indices.filter { insns[it].opcode?.isReturn == true }
    for (index in returns.asReversed()) {
        val captures = if (target.returnType == Type.Void) emptyList() else listOf(Capture("result", target.returnType, target.registerA(index)))
        target.insertCode(index, captures, block)
    }
}

/** Replaces the method body with `block`. */
fun MethodTarget.replace(block: CodeScope.() -> Unit) = method.replaceCode(block)

fun MethodTarget.alwaysReturn() = method.alwaysReturn()
fun MethodTarget.alwaysReturn(value: Boolean) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: Int) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: Long) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: String) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturnNull() = method.alwaysReturnNull()

fun MethodTarget.replaceAllStrings(old: String, new: String): Int = method.replaceAllStrings(old, new)
fun MethodTarget.replaceAllLiterals(old: Long, new: Long): Int = method.replaceAllLiterals(old, new)

/** Runs `block` just before the instruction at the point. */
fun PointTarget.before(block: CodeScope.() -> Unit) {
    val point = resolved
    point.method.insertCode(point.index, point.captures, block)
}

/** Runs `block` just after the instruction at the point. */
fun PointTarget.after(block: CodeScope.() -> Unit) {
    val point = resolved
    point.method.insertCode(minOf(point.index + 1, point.method.instructionCount), point.captures, block)
}

internal fun Method.insertCode(index: Int, captures: List<Capture>, block: CodeScope.() -> Unit) {
    val emitter = CodeEmitter.forInsertion(this, index, captures)
    emitter.block()
    val compiled = emitter.buildInsertion()
    if (compiled.localGrowth > 0) {
        require(growLocalRegisters(compiled.localGrowth)) {
            "Cannot insert code in $descriptor[$index]: failed to grow method locals by ${compiled.localGrowth}"
        }
    }
    insertInstructions(index, compiled.instructions)
}

internal fun Method.replaceCode(block: CodeScope.() -> Unit) {
    val emitter = CodeEmitter.forReplacement(this)
    emitter.block()
    val plan = emitter.buildReplacement()
    replaceBody(plan.registersSize, plan.outsSize, plan.instructions)
}
