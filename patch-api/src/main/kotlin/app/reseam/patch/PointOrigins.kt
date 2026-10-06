// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.invokeArgumentTypes
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.regA
import app.reseam.patch.dex.regB
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.native.registerWriters
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
        val call = insns[at]
        val types = call.invokeArgumentTypes ?: return null
        if (argument !in types.indices) return null
        val value = call.arguments("${method.descriptor}[$at]")[argument]
        val origins = trace(insns, at, value.register, before, mutableSetOf())
        if (registerWordCount(value.type) == 2) {
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

    /** Resolves a staged argument to the register holding its value before the anchored block. */
    fun atEntry(
        insns: List<Instruction>,
        at: Int,
        argument: Int,
        entry: Int,
        label: String,
    ): InvokeArgument {
        val value =
            insns[at].arguments(label).getOrNull(argument)
                ?: error("$label: invoke has no argument $argument")
        if (entry == at) return value
        val origins =
            requireNotNull(argument(insns, at, argument, entry)) {
                "$label: invoke has no argument $argument"
            }
        val register = origins.registersAtBoundary.singleOrNull()
        require(register != null && !origins.unknown && origins.sites.isEmpty()) {
            "$label: argument $argument has no unique value before instruction $entry"
        }
        return InvokeArgument(register, value.type)
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
        val writers = registerWriters(method.handle, at.toUInt(), register.toUShort())
        val result =
            if (writers == null) Origins(emptySet(), unknown = true)
            else {
                val sources =
                    writers.indices.map { raw ->
                        val index = raw.toInt()
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
