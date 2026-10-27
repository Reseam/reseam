// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.Instruction

/**
 * Rewrites `invoke-kind` instructions the 35c format cannot encode (more than five registers, or a
 * register above v15) into `invoke-kind/range`, moving the arguments into scratch registers when
 * they are not consecutive.
 */
fun lowerInvokes(
    insns: List<Instruction>,
    scratch: (wordCount: Int) -> List<Int>,
): List<Instruction> {
    val counts = app.reseam.patch.native.invokeScratchWords(insns)
    val spans =
        insns.indices.mapNotNull { index ->
            val words = counts[index].toInt()
            if (words == 0) null
            else {
                val registers = scratch(words)
                require(registers.all { it in 0..UShort.MAX_VALUE.toInt() }) {
                    "scratch registers must fit the DEX frame"
                }
                app.reseam.patch.types.ScratchSpan(
                    index.toUInt(),
                    UShortArray(registers.size) { registers[it].toUShort() },
                )
            }
        }
    return app.reseam.patch.native.lowerInstructions(insns, spans)
}

val Opcode.rangeVariant: Opcode?
    get() =
        when (this) {
            Opcode.INVOKE_VIRTUAL -> Opcode.INVOKE_VIRTUAL_RANGE
            Opcode.INVOKE_SUPER -> Opcode.INVOKE_SUPER_RANGE
            Opcode.INVOKE_DIRECT -> Opcode.INVOKE_DIRECT_RANGE
            Opcode.INVOKE_STATIC -> Opcode.INVOKE_STATIC_RANGE
            Opcode.INVOKE_INTERFACE -> Opcode.INVOKE_INTERFACE_RANGE
            else -> null
        }

internal fun contiguousUnusedRegisters(
    registerCount: Int,
    excluded: Set<Int>,
    wordCount: Int,
): List<Int>? {
    if (wordCount == 0) return emptyList()
    return (0..registerCount - wordCount)
        .map { start -> (start until start + wordCount).toList() }
        .firstOrNull { candidate -> candidate.none { it in excluded } }
}
