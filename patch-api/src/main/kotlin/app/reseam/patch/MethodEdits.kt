// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.types.MethodEdit

internal class InstructionAnchor(private val label: String, index: Int) {
    private var headIndex = index
    private var instructionIndex = index
    private var tailIndex = index + 1
    private var lost: String? = null

    val head: Int
        get() = live().headIndex

    val instruction: Int
        get() = live().instructionIndex

    val tail: Int
        get() = live().tailIndex

    private fun live(): InstructionAnchor {
        lost?.let { error("$label: $it") }
        return this
    }

    fun relocate(mapping: MethodEdit, kind: EditKind, removed: IntRange) {
        if (lost != null) return
        if (instructionIndex in removed) {
            lose("the instruction it names was removed from the method")
            return
        }
        headIndex =
            when (kind) {
                EditKind.INSERT -> mapping.instructions[headIndex].toInt()
                EditKind.GROW,
                EditKind.REPLACE,
                EditKind.REMOVE -> mapping.starts[headIndex].toInt()
            }
        instructionIndex = mapping.instructions[instructionIndex].toInt()
        tailIndex =
            when (kind) {
                EditKind.INSERT -> mapping.instructions[tailIndex].toInt()
                EditKind.GROW,
                EditKind.REPLACE,
                EditKind.REMOVE -> mapping.ends[tailIndex - 1].toInt()
            }
    }

    fun lose(reason: String) {
        if (lost == null) lost = reason
    }
}

internal enum class EditKind {
    INSERT,
    REPLACE,
    REMOVE,
    GROW,
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

internal class MethodEdits {
    private val tracked = HashMap<UInt, Tracked>()

    private class Tracked {
        var prologueEnd = 0
        val anchors = mutableListOf<InstructionAnchor>()
        val captures = mutableListOf<Capture>()
        val locals = mutableListOf<MethodLocal>()
    }

    private fun of(handle: UInt): Tracked = tracked.getOrPut(handle) { Tracked() }

    fun anchor(method: Method, index: Int, label: String): InstructionAnchor =
        InstructionAnchor(label, index).also { of(method.handle).anchors += it }

    fun track(method: Method, capture: Capture) {
        of(method.handle).captures += capture
    }

    fun reserve(local: MethodLocal) {
        of(local.method.handle).locals += local
    }

    fun reserved(method: Method): List<MethodLocal> = tracked[method.handle]?.locals.orEmpty()

    fun entry(method: Method): Int = tracked[method.handle]?.prologueEnd ?: 0

    fun initialized(method: Method, count: Int) {
        of(method.handle).prologueEnd += count
    }

    fun relocated(
        handle: UInt,
        mapping: MethodEdit,
        kind: EditKind,
        removed: IntRange = IntRange.EMPTY,
    ) {
        val entry = tracked[handle] ?: return
        entry.prologueEnd = mapping.starts[entry.prologueEnd].toInt()
        entry.anchors.forEach { it.relocate(mapping, kind, removed) }
    }

    /** Locals added below the incoming window shift every register at or above [base]. */
    fun grewRegisters(handle: UInt, base: Int, additional: Int) {
        val entry = tracked[handle] ?: return
        entry.captures.forEach { it.relocate(base, additional) }
        entry.locals.forEach { it.relocate(base, additional) }
    }

    fun bodyReplaced(handle: UInt) {
        val entry = tracked[handle] ?: return
        entry.anchors.forEach { it.lose("the method body was replaced under it") }
        entry.anchors.clear()
        entry.prologueEnd = 0
        entry.captures.clear()
        entry.locals.forEach { it.invalidate() }
        entry.locals.clear()
    }
}

internal val Method.edits: MethodEdits?
    get() = ActiveRuntime.currentOrNull?.edits
