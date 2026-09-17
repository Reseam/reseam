// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.invokeArgumentTypes
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.regB
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.native.Instruction
import app.reseam.patch.native.registerWriters

/** Scoped to one unmodified method snapshot; never reused after mutation. */
internal class ArgumentOrigins(private val method: Method) {
    private data class Origins(val sites: Set<Int>, val entry: Boolean = false, val unknown: Boolean = false)
    private val cache = mutableMapOf<Pair<Int, Int>, Origins>()
    val diagnostics = linkedSetOf<String>()

    fun matches(insns: List<Instruction>, at: Int, argument: Int, constraint: PointStep): Boolean {
        val call = insns[at]
        val types = call.invokeArgumentTypes ?: return false
        if (argument !in types.indices) return false
        val value = call.arguments("${method.descriptor}[$at]")[argument]
        val origins = trace(insns, at, value.register, mutableSetOf())
        if (registerWordCount(value.type) == 2 && origins != trace(insns, at, value.register + 1, mutableSetOf())) {
            diagnostics += "${method.descriptor}[$at] argument($argument): inconsistent wide value sources"
            return false
        }
        if (origins.unknown || origins.entry || origins.sites.isEmpty()) {
            val reason = when {
                origins.unknown -> "unknown control flow or cyclic copy origin"
                origins.entry -> "includes an incoming parameter or undefined entry value"
                else -> "unreachable argument"
            }
            diagnostics += "${method.descriptor}[$at] argument($argument): $reason"
            return false
        }
        return origins.sites.all { constraint.matches(insns, it) }
    }

    private fun trace(insns: List<Instruction>, at: Int, register: Int, visiting: MutableSet<Pair<Int, Int>>): Origins {
        val key = at to register
        cache[key]?.let { return it }
        if (!visiting.add(key)) return Origins(emptySet(), unknown = true)
        val writers = registerWriters(method.handle, at.toUInt(), register.toUShort())
        val result = if (writers == null) Origins(emptySet(), unknown = true) else {
            val sources = writers.indices.map { raw ->
                val index = raw.toInt()
                val insn = insns[index]
                if (insn.opcode in copies) {
                    val source = insn.regB
                    if (source == null) Origins(emptySet(), unknown = true)
                    else trace(insns, index, source + (register - (insn.regA ?: register)), visiting)
                } else Origins(setOf(index))
            }
            Origins(sources.flatMapTo(linkedSetOf()) { it.sites }, writers.fromEntry || sources.any { it.entry }, sources.any { it.unknown })
        }
        visiting.remove(key)
        cache[key] = result
        return result
    }

    private companion object {
        val copies = setOf(Opcode.MOVE, Opcode.MOVE_FROM16, Opcode.MOVE_16,
            Opcode.MOVE_OBJECT, Opcode.MOVE_OBJECT_FROM16, Opcode.MOVE_OBJECT_16,
            Opcode.MOVE_WIDE, Opcode.MOVE_WIDE_FROM16, Opcode.MOVE_WIDE_16)
    }
}
