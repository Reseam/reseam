// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.descriptor
import app.reseam.patch.types.Instruction

internal fun CodeEmitter.buildReplacement(): ReplacementPlan {
    // Trailing labels may otherwise leave branches targeting past the method body.
    val trailingLabels = ops.takeLastWhile { it.label != null }.mapNotNull { it.label }.toSet()
    check(
        ops.lastOrNull { it.label == null }?.fallsThrough == false &&
            ops.none { it.target in trailingLabels }
    ) {
        "The replacement for ${info.descriptor} falls off the end of the method; end every path with a return"
    }
    val instructions = build()
    return ReplacementPlan(replacementLocals + incomingWords, maxOutRegisters, instructions)
}

internal fun CodeEmitter.build(): List<Instruction> {
    layoutRegisters()
    val builder = InstructionBuilder()
    if (mode == EmissionMode.REPLACEMENT)
        for ((offset, value) in entryValues) {
            val allocation =
                tempAllocations.getValue((value.register as Register.Temporary).allocation)
            if (allocation.stageIncoming)
                builder.moveTyped(
                    checkNotNull(allocation.baseRegister),
                    replacementLocals + offset,
                    value.type,
                )
        }
    for (op in ops) op.emit(builder, ::resolveRegister)
    return builder.build()
}

internal fun CodeEmitter.op(
    reads: List<Register> = emptyList(),
    writes: List<Register> = emptyList(),
    label: EmissionLabel? = null,
    target: EmissionLabel? = null,
    fallsThrough: Boolean = true,
    emit: Emit,
): Op = Op(reads, writes, label, target, fallsThrough, emit).also { ops += it }

internal fun CodeEmitter.allocTemp(
    wordCount: Int = 1,
    constraint: RegisterConstraint = RegisterConstraint.BYTE,
): Register.Temporary {
    require(wordCount > 0)
    val tempId = TemporaryId(nextTempId++)
    tempAllocations[tempId] = TempAllocation(wordCount, constraint)
    return Register.Temporary(tempId)
}

internal fun CodeEmitter.low(register: Int, context: String): Int {
    require(register in 0..15) { "$context requires a 4-bit register, got v$register" }
    return register
}

internal fun CodeEmitter.byte(register: Int, context: String): Int {
    require(register in 0..0xFF) { "$context requires an 8-bit register, got v$register" }
    return register
}
