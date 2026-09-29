// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.buildInstructions
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.rangeVariant
import app.reseam.patch.native.appEntryHook

/** Runs `block` when the method is entered. */
fun MethodTarget.before(block: CodeScope.() -> Unit) = method.insertCode(method.edits?.entry(method) ?: 0, emptyList(), block)

/**
 * Runs `block` before every return; `capture("result")` is the value being returned.
 * Referenced parameters and `thisObject` are saved at entry, before the body can reuse their registers.
 */
fun MethodTarget.after(block: CodeScope.() -> Unit) {
    val target = method
    val insns = target.instructions
    val returns = insns.indices.filter { insns[it].opcode?.isReturn == true }
    val snapshots = mutableMapOf<Int, EntrySnapshot>()
    for (ordinal in returns.indices.reversed()) {
        // Lowering during frame growth can expand earlier instructions. Newly
        // emitted returns follow every original return still to be processed.
        val current = target.instructions
        val index = current.indices.filter { current[it].opcode?.isReturn == true }[ordinal]
        val captures = if (target.returnType == Type.Void) emptyList() else listOf(Capture("result", target.returnType, target.registerA(index)))
        target.insertCode(index, captures, block, snapshots)
    }
    if (snapshots.isNotEmpty()) {
        val incomingBase = target.registersSize - target.insSize
        target.insertInstructions(0, buildInstructions {
            for (snapshot in snapshots.values) {
                moveTyped(snapshot.register, incomingBase + snapshot.offset, snapshot.type)
            }
        })
    }
}

/** Code [appEntry] adds: a [CodeScope] with the app's `Application` as [application]. */
interface AppEntryScope : CodeScope {
    val application: ValueRef
}

/**
 * Runs `block` once at process start, first in `onCreate()` of the app's `Application`.
 * The engine binds it to the class the manifest names after every patch has run, so a
 * patch that swaps that class does not strand it. Fails when the manifest names none.
 */
fun appEntry(block: AppEntryScope.() -> Unit) {
    val hook = Method(appEntryHook())
    hook.insertCode(hook.edits?.entry(hook) ?: 0, emptyList(), { AppEntryCode(this).block() })
}

private class AppEntryCode(code: CodeScope) : AppEntryScope, CodeScope by code {
    override val application: ValueRef = code.param(0)
    override val thisObject: ValueRef
        get() = error("appEntry code runs in a static hook; use application")
}

/** Replaces the method body with `block`. */
fun MethodTarget.replace(block: CodeScope.() -> Unit) = method.replaceCode(block)

/**
 * Reserves a register shared across this method's code blocks, accessed as `local(slot)`.
 * Grows the frame and initializes the register to zero or null before entry hooks.
 */
fun MethodTarget.reserveLocal(name: String, type: String): MethodLocal {
    val target = method
    val local = MethodLocal(name, descriptor(type), target)
    val register = target.registersSize - target.insSize
    require(register <= 0xFF) {
        "Cannot reserve '$name' in ${target.descriptor}: v$register is past the 8-bit register a constant can name"
    }
    requireNotNull(target.growLocals(local.wordCount)) {
        "Cannot reserve '$name' in ${target.descriptor}: the frame would not grow by ${local.wordCount}"
    }
    local.register = register
    ActiveRuntime.current.edits.reserve(local)
    val initializer = buildInstructions {
        if (local.wordCount == 2) constLong(register, 0) else constInt(register, 0)
    }
    target.insertInstructions(ActiveRuntime.current.edits.entry(target), initializer)
    ActiveRuntime.current.edits.initialized(target, initializer.size)
    return local
}

fun MethodTarget.alwaysReturn() = method.alwaysReturn()
fun MethodTarget.alwaysReturn(value: Boolean) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: Int) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: Long) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturn(value: String) = method.alwaysReturn(value)
fun MethodTarget.alwaysReturnNull() = method.alwaysReturnNull()

fun MethodTarget.replaceAllStrings(old: String, new: String): Int = method.replaceAllStrings(old, new)
fun MethodTarget.replaceAllLiterals(old: Long, new: Long): Int = method.replaceAllLiterals(old, new)

/**
 * Runs `block` just before the instruction at the point, after anything an earlier block put there.
 * Branches to the instruction run it too.
 */
fun PointTarget.before(block: CodeScope.() -> Unit) {
    val point = resolved
    point.method.insertCode(point.anchor.head, point.captures, block, everyPath = true)
}

/** Runs `block` just after the instruction at the point, after anything an earlier block put there. */
fun PointTarget.after(block: CodeScope.() -> Unit) {
    val point = resolved
    val opcode = point.method.instructions[point.index].opcode
    require(opcode?.endsFlow != true) {
        "$label: nothing runs after the $opcode at ${point.method.descriptor}[${point.index}]; use before"
    }
    point.method.insertCode(minOf(point.anchor.tail, point.method.instructionCount), point.captures, block)
}

/**
 * Runs the call only when `condition` is false, including on branches targeting the call.
 * The result must be unused: a skipped call cannot supply a following `move-result`.
 */
fun PointTarget.skipWhen(condition: CodeScope.() -> ValueRef) {
    val point = resolved
    val method = point.method
    val call = method.instructions[point.index]
    val callOpcode = call.opcode?.takeIf { it.isInvoke }
        ?: error("$label: skipWhen needs a call, got ${call.opcode} at ${method.descriptor}[${point.index}]")
    val ref = call.methodRef ?: error("$label: the call at ${method.descriptor}[${point.index}] names no method")
    require(method.instructions.getOrNull(point.index + 1)?.opcode?.isMoveResult != true) {
        "$label: the result of ${ref.descriptor} is used; skipWhen needs a call whose result is unused"
    }
    val opcode = Opcode.entries.firstOrNull { it.rangeVariant == callOpcode } ?: callOpcode
    val arguments = call.arguments("$label: ${method.descriptor}[${point.index}]").mapIndexed { i, arg ->
        Capture("skipWhen$i", arg.type, arg.register)
    }
    method.insertCode(
        point.anchor.head,
        point.captures + arguments,
        { whenFalse(condition()) { (this as CodeEmitter).callCaptured(opcode, ref, arguments.map { it.name }) } },
        everyPath = true,
    )
    method.removeInstruction(point.anchor.head)
}

internal fun Method.insertCode(
    index: Int,
    captures: List<Capture>,
    block: CodeScope.() -> Unit,
    entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
    everyPath: Boolean = entrySnapshots != null,
) {
    val emitter = CodeEmitter.forInsertion(this, index, captures, entrySnapshots)
    emitter.block()
    val compiled = emitter.buildInsertion()
    val insertionIndex = if (compiled.localGrowth > 0) {
        val indices = requireNotNull(growLocals(compiled.localGrowth)) {
            "Cannot insert code in $descriptor[$index]: failed to grow method locals by ${compiled.localGrowth}"
        }
        indices[index]
    } else index
    if (everyPath) {
        check(insertOnEveryPath(insertionIndex, compiled.instructions)) {
            "Cannot insert code on every path into $descriptor[$index]"
        }
    } else {
        insertInstructions(insertionIndex, compiled.instructions)
    }
}

internal fun Method.replaceCode(block: CodeScope.() -> Unit) {
    val emitter = CodeEmitter.forReplacement(this)
    emitter.block()
    val plan = emitter.buildReplacement()
    replaceBody(plan.registersSize, plan.outsSize, plan.instructions)
}
