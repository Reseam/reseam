// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.points

import app.reseam.patch.Type
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.OperandType
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.operands
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.regB
import app.reseam.patch.dex.writers
import app.reseam.patch.types.Instruction

internal class ArgumentOrigins(private val method: Method) {
    data class Origins(
        val sites: Set<Int>,
        val entry: Boolean = false,
        val unknown: Boolean = false,
        val registersAtBoundary: Set<Int> = emptySet(),
    )

    private data class TraceKey(val instruction: Int, val register: Int, val before: Int?)

    private val cache = mutableMapOf<TraceKey, Origins>()
    val diagnostics = linkedSetOf<String>()

    fun argument(insns: List<Instruction>, at: Int, argument: Int, before: Int? = null): Origins? {
        val value =
            insns[at].operands("${method.descriptor}[$at]")?.getOrNull(argument) ?: return null
        val origins = trace(insns, at, value.register, before, mutableSetOf())
        if (value.words == 2) {
            val high = trace(insns, at, value.register + 1, before, mutableSetOf())
            if (
                origins.sites != high.sites ||
                    origins.entry != high.entry ||
                    origins.unknown != high.unknown ||
                    origins.registersAtBoundary.mapTo(mutableSetOf()) { it + 1 } !=
                        high.registersAtBoundary
            ) {
                diagnostics +=
                    "${method.descriptor}[$at] argument($argument): inconsistent wide value sources"
                return Origins(emptySet(), unknown = true)
            }
        }
        return origins
    }

    /** Where the value in [register] at instruction [at] was written. */
    fun register(insns: List<Instruction>, at: Int, register: Int): Origins =
        trace(insns, at, register, null, mutableSetOf())

    /** Resolves a staged operand to the register holding its value before the anchored block. */
    fun atEntry(insns: List<Instruction>, at: Int, argument: Int, entry: Int, label: String): Int {
        val value =
            insns[at].operands(label)?.getOrNull(argument)
                ?: error("$label: ${insns[at].opcode} has no argument $argument")
        if (entry == at) return value.register
        val origins = checkNotNull(argument(insns, at, argument, entry))
        val register = origins.registersAtBoundary.singleOrNull()
        require(register != null && !origins.unknown && origins.sites.isEmpty()) {
            "$label: argument $argument has no unique value before instruction $entry"
        }
        return register
    }

    fun operandTypes(insns: List<Instruction>, at: Int): List<String?>? =
        insns[at].operands("${method.descriptor}[$at]")?.map { operand ->
            when (val type = operand.type) {
                is OperandType.Stated -> type.descriptor
                OperandType.Written -> type(insns, at, operand.register)
                OperandType.Array -> arrayType(insns, at)
                OperandType.Element -> elementType(insns, at)
                OperandType.Returned -> method.returnType
            }
        }

    /** The array type of the array access at [at]. */
    fun arrayType(insns: List<Instruction>, at: Int): String? =
        type(insns, at, insns[at].regB!!)?.takeIf { it.startsWith('[') }
            ?: elementByOpcode[insns[at].opcode]?.let { "[$it" }

    /** The element type the array access at [at] moves. */
    fun elementType(insns: List<Instruction>, at: Int): String? =
        elementByOpcode[insns[at].opcode] ?: arrayType(insns, at)?.substring(1)

    /** The type of [register] at [at], when every instruction or parameter writing it agrees. */
    fun type(insns: List<Instruction>, at: Int, register: Int): String? {
        val origins = trace(insns, at, register, 0, mutableSetOf())
        if (origins.unknown) return null
        val incoming = method.incomingTypes
        val types =
            origins.sites.map { inferType(method, it, this) } +
                origins.registersAtBoundary.map { method.incomingAt(it)?.let(incoming::get) }
        return types.distinct().singleOrNull()
    }

    fun matches(insns: List<Instruction>, at: Int, argument: Int, constraint: PointStep): Boolean {
        val origins = argument(insns, at, argument) ?: return false
        if (origins.unknown || origins.entry || origins.sites.isEmpty()) {
            val reason =
                when {
                    origins.unknown -> "unknown control flow or cyclic copy origin"
                    origins.entry -> "includes an incoming parameter or undefined entry value"
                    else -> "unreachable argument"
                }
            diagnostics += "${method.descriptor}[$at] argument($argument): $reason"
            return false
        }
        return origins.sites.all { constraint.matches(insns, it) }
    }

    private fun trace(
        insns: List<Instruction>,
        at: Int,
        register: Int,
        before: Int?,
        visiting: MutableSet<TraceKey>,
    ): Origins {
        val key = TraceKey(at, register, before)
        cache[key]?.let {
            return it
        }
        if (!visiting.add(key)) return Origins(emptySet(), unknown = true)
        val writers = method.writers(at, register)
        val result =
            if (writers == null) Origins(emptySet(), unknown = true)
            else {
                val sources =
                    writers.indices.map { index ->
                        val insn = insns[index]
                        if (before != null && index < before) {
                            Origins(emptySet(), entry = true, registersAtBoundary = setOf(register))
                        } else if (insn.opcode in copies) {
                            val source = insn.regB
                            if (source == null) Origins(emptySet(), unknown = true)
                            else
                                trace(
                                    insns,
                                    index,
                                    source + (register - (insn.regA ?: register)),
                                    before,
                                    visiting,
                                )
                        } else Origins(setOf(index))
                    }
                Origins(
                    sources.flatMapTo(linkedSetOf()) { it.sites },
                    writers.fromEntry || sources.any { it.entry },
                    sources.any { it.unknown },
                    sources
                        .flatMapTo(linkedSetOf()) { it.registersAtBoundary }
                        .apply {
                            if (writers.fromEntry && before != null) add(register)
                        },
                )
            }
        visiting.remove(key)
        cache[key] = result
        return result
    }

    private companion object {
        val elementByOpcode =
            mapOf(
                Opcode.AGET_BOOLEAN to Type.Boolean,
                Opcode.APUT_BOOLEAN to Type.Boolean,
                Opcode.AGET_BYTE to Type.Byte,
                Opcode.APUT_BYTE to Type.Byte,
                Opcode.AGET_CHAR to Type.Char,
                Opcode.APUT_CHAR to Type.Char,
                Opcode.AGET_SHORT to Type.Short,
                Opcode.APUT_SHORT to Type.Short,
            )

        val copies =
            setOf(
                Opcode.MOVE,
                Opcode.MOVE_FROM16,
                Opcode.MOVE_16,
                Opcode.MOVE_OBJECT,
                Opcode.MOVE_OBJECT_FROM16,
                Opcode.MOVE_OBJECT_16,
                Opcode.MOVE_WIDE,
                Opcode.MOVE_WIDE_FROM16,
                Opcode.MOVE_WIDE_16,
            )
    }
}
