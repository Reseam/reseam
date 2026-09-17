// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.registerWordCount

/**
 * Tracks an instruction through method edits. [head] and [tail] are insertion
 * boundaries that keep repeated before/after emissions in patch order.
 */
internal class InstructionAnchor(private val label: String, index: Int) {
    private var headIndex = index
    private var tailIndex = index + 1
    private var lost: String? = null

    val head: Int get() = live().headIndex
    val tail: Int get() = live().tailIndex

    private fun live(): InstructionAnchor {
        lost?.let { error("$label: $it") }
        return this
    }

    fun shift(insertedAt: Int, count: Int) {
        if (count == 0) return
        if (headIndex >= insertedAt) headIndex += count
        if (tailIndex >= insertedAt) tailIndex += count
    }

    fun shiftAfter(index: Int, count: Int) {
        if (count == 0) return
        if (headIndex > index) headIndex += count
        if (tailIndex > index) tailIndex += count
    }

    fun drop(removedAt: Int, count: Int) {
        if (count == 0) return
        if (headIndex in removedAt until removedAt + count) {
            lose("the instruction it names was removed from the method")
            return
        }
        headIndex = afterRemoval(headIndex, removedAt, count)
        tailIndex = maxOf(headIndex + 1, afterRemoval(tailIndex, removedAt, count))
    }

    /** Relocates against a mapping of every original instruction boundary to its new index. */
    fun relocate(boundaries: List<Int>, expandedAt: Int, expandedBy: Int) {
        if (lost != null) return
        val at = { index: Int -> boundaries[index] + if (index == expandedAt) expandedBy else 0 }
        headIndex = at(headIndex)
        tailIndex = maxOf(headIndex + 1, at(tailIndex))
    }

    fun lose(reason: String) {
        if (lost == null) lost = reason
    }

    private fun afterRemoval(index: Int, removedAt: Int, count: Int): Int = when {
        index >= removedAt + count -> index - count
        index > removedAt -> removedAt
        else -> index
    }
}

/**
 * A method-wide register reserved by [MethodTarget.reserveLocal], accessed as `local(slot)`.
 */
class MethodLocal internal constructor(val name: String, val type: String, internal val method: Method) {
    internal var register: Int = -1
    internal var lost: Boolean = false
    internal val wordCount: Int = registerWordCount(type)

    override fun toString(): String = "$name: $type in ${method.descriptor}"
}

/**
 * Relocates anchors, captures and reserved locals after edits reported by [Method].
 */
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

    fun inserted(handle: UInt, index: Int, count: Int) {
        val entry = tracked[handle] ?: return
        if (index < entry.prologueEnd) entry.prologueEnd += count
        entry.anchors.forEach { it.shift(index, count) }
    }

    fun removed(handle: UInt, index: Int, count: Int) {
        val entry = tracked[handle] ?: return
        entry.prologueEnd -= minOf(count, maxOf(0, entry.prologueEnd - index))
        entry.anchors.forEach { it.drop(index, count) }
    }

    fun replaced(handle: UInt, index: Int, growth: Int) {
        val entry = tracked[handle] ?: return
        if (index < entry.prologueEnd) entry.prologueEnd += growth
        entry.anchors.forEach { it.shiftAfter(index, growth) }
    }

    fun relocated(handle: UInt, boundaries: List<Int>, expandedAt: Int = -1, expandedBy: Int = 0) {
        val entry = tracked[handle] ?: return
        entry.prologueEnd = boundaries[entry.prologueEnd]
        entry.anchors.forEach { it.relocate(boundaries, expandedAt, expandedBy) }
    }

    /** Locals added below the incoming window shift every register at or above [base]. */
    fun grewRegisters(handle: UInt, base: Int, additional: Int) {
        val entry = tracked[handle] ?: return
        for (capture in entry.captures) if (capture.register >= base) capture.register += additional
        for (local in entry.locals) if (local.register >= base) local.register += additional
    }

    fun bodyReplaced(handle: UInt) {
        val entry = tracked[handle] ?: return
        entry.anchors.forEach { it.lose("the method body was replaced under it") }
        entry.anchors.clear()
        entry.prologueEnd = 0
        entry.locals.forEach { it.lost = true }
        entry.locals.clear()
    }
}

internal val Method.edits: MethodEdits? get() = ActiveRuntime.currentOrNull?.edits
