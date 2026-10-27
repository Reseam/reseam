// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.arguments
import app.reseam.patch.dex.buildInstructions
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.growLocals
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.emit.EmittedCode
import app.reseam.patch.emit.builtBy
import app.reseam.patch.emit.emitInvoke
import app.reseam.patch.emit.insertCode
import app.reseam.patch.native.appEntryHook
import app.reseam.patch.points.ArgumentOrigins
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.run.edits
import app.reseam.patch.types.CodePlacement

/** Runs `block` when the method is entered. */
fun MethodTarget.before(block: CodeScope.() -> Unit) = method.insertCode(method.entry, block)

/** The start of the method, past the code that initializes its reserved locals. */
private val Method.entry: CodePlacement
    get() = CodePlacement.Before((edits?.entry(this) ?: 0).toUInt())

/**
 * Runs `block` before every return; [CodeScope.result] is the value being returned. Referenced
 * parameters and `thisObject` are saved at entry, before the body can reuse their registers.
 */
fun MethodTarget.after(block: CodeScope.() -> Unit) =
    method.insertCode(CodePlacement.BeforeReturns, block)

/** Code [appEntry] adds: a [CodeScope] with the app's `Application` as [application]. */
interface AppEntryScope : CodeScope {
    val application: ValueRef
}

/**
 * Runs `block` once at process start, first in `onCreate()` of the app's `Application`. The engine
 * binds it to the class the manifest names after every patch has run, so a patch that swaps that
 * class does not strand it. Fails when the manifest names none.
 */
fun appEntry(block: AppEntryScope.() -> Unit) {
    val hook = Method(appEntryHook())
    hook.insertCode(hook.entry, { AppEntryCode(this).block() })
}

private class AppEntryCode(code: CodeScope) : AppEntryScope, CodeScope by code {
    override val application: ValueRef = code.param(0)
    override val thisObject: ValueRef
        get() = error("appEntry code runs in a static hook; use application")
}

/** Replaces the method body with `block`. */
fun MethodTarget.replace(block: CodeScope.() -> Unit) = method.insertCode(CodePlacement.Body, block)

/**
 * Reserves a register shared across this method's code blocks, accessed as `local(slot)`. Grows the
 * frame and initializes the register to zero or null before entry hooks.
 */
fun MethodTarget.reserveLocal(name: String, type: String): MethodLocal {
    val target = method
    val register = target.registersSize - target.insSize
    val local = MethodLocal(name, descriptor(type), target, register)
    require(register <= 0xFF) {
        "Cannot reserve '$name' in ${target.descriptor}: v$register is past the 8-bit register a constant can name"
    }
    target.growLocals(local.wordCount)
    ActiveRuntime.current.edits.reserve(local)
    val initializer = buildInstructions {
        if (local.wordCount == 2) constLong(register, 0) else constInt(register, 0)
    }
    target.insertInstructions(ActiveRuntime.current.edits.entry(target), initializer)
    ActiveRuntime.current.edits.initialized(target, initializer.size)
    return local
}

/** A method-wide register reserved by [MethodTarget.reserveLocal], accessed as `local(slot)`. */
class MethodLocal
internal constructor(
    val name: String,
    val type: String,
    internal val method: Method,
    register: Int,
) {
    internal var register: Int = register
        private set

    internal var lost: Boolean = false
        private set

    internal fun relocate(base: Int, additional: Int) {
        if (register >= base) register += additional
    }

    internal fun invalidate() {
        lost = true
    }

    internal val wordCount: Int = registerWordCount(type)

    override fun toString(): String = "$name: $type in ${method.descriptor}"
}

/** Replaces the method body with `return`. */
fun MethodTarget.alwaysReturn() = replace { returnVoid() }

/** Replaces the method body with `return value`. */
fun MethodTarget.alwaysReturn(value: Boolean) = replace { returnValue(bool(value)) }

/** Replaces the method body with `return value`. */
fun MethodTarget.alwaysReturn(value: Int) = replace { returnValue(int(value)) }

/** Replaces the method body with `return value`. */
fun MethodTarget.alwaysReturn(value: Long) = replace { returnValue(long(value)) }

/** Replaces the method body with `return value`. */
fun MethodTarget.alwaysReturn(value: String) = replace { returnValue(string(value)) }

/** Replaces the method body with `return null`. */
fun MethodTarget.alwaysReturnNull() = replace { returnNull() }

/**
 * Runs `block` just before the instruction at the point, after anything an earlier block put there.
 * Branches to the instruction run it too.
 */
fun PointTarget.before(block: CodeScope.() -> Unit) {
    val point = resolved
    point.method.insertCode(
        CodePlacement.OnEveryPath(point.anchor.head.toUInt()),
        block,
        point.captures,
    )
}

/**
 * Runs `block` just after the instruction at the point, after anything an earlier block put there.
 */
fun PointTarget.after(block: CodeScope.() -> Unit) {
    val point = resolved
    val opcode = point.method.instructions[point.index].opcode
    require(opcode?.endsFlow != true) {
        "$label: nothing runs after the $opcode at ${point.method.descriptor}[${point.index}]; use before"
    }
    point.method.insertCode(
        CodePlacement.Before(minOf(point.anchor.tail, point.method.instructionCount).toUInt()),
        block,
        point.captures,
    )
}

/**
 * Runs the call only when `condition` is false, including on branches targeting the call. The
 * result must be unused: a skipped call cannot supply a following `move-result`.
 */
fun PointTarget.skipWhen(condition: CodeScope.() -> ValueRef) {
    val point = resolved
    val method = point.method
    val call = method.instructions[point.index]
    val callOpcode =
        call.opcode?.takeIf { it.isInvoke }
            ?: error(
                "$label: skipWhen needs a call, got ${call.opcode} at ${method.descriptor}[${point.index}]"
            )
    val ref =
        call.methodRef
            ?: error("$label: the call at ${method.descriptor}[${point.index}] names no method")
    require(method.instructions.getOrNull(point.index + 1)?.opcode?.isMoveResult != true) {
        "$label: the result of ${ref.descriptor} is used; skipWhen needs a call whose result is unused"
    }
    val origins = ArgumentOrigins(method)
    val arguments =
        call.arguments("$label: ${method.descriptor}[${point.index}]").mapIndexed { i, arg ->
            val original =
                origins.atEntry(method.instructions, point.index, i, point.anchor.head, label)
            Capture("skipWhen$i", arg.type, original.register)
        }
    method.insertCode(
        CodePlacement.OnEveryPath(point.anchor.head.toUInt()),
        {
            whenFalse(condition()) {
                val emitter = (this as EmittedCode).emitter
                emitter.emitInvoke(
                    callOpcode,
                    ref,
                    arguments.map { capture(it.name).builtBy(emitter) },
                )
            }
        },
        point.captures + arguments,
    )
    method.removeInstructions(point.anchor.head, point.anchor.tail - point.anchor.head)
}
