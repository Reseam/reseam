// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Opcode
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodRef

internal const val MIN_REPLACEMENT_LOCALS = 16

@JvmInline internal value class TemporaryId(val index: Int)

internal sealed interface Register {
    data class Physical(val index: Int) : Register

    data class Temporary(val allocation: TemporaryId, val word: Int = 0) : Register

    data object Void : Register
}

@JvmInline
internal value class EmissionLabel(val index: Int) {
    val name: String
        get() = "label_$index"
}

internal class ReplacementPlan(
    val registersSize: Int,
    val outsSize: Int,
    val instructions: List<Instruction>,
)

internal enum class RegisterConstraint(val maxRegister: Int) {
    LOW(15),
    BYTE(0xFF),
    ANY(0xFFFF),
}

internal class EntrySnapshot(val offset: Int, val register: Int, val type: String)

internal class TempAllocation(
    val wordCount: Int,
    var constraint: RegisterConstraint,
    val entryOffset: Int? = null,
    val incomingOffset: Int? = null,
) {
    var stageIncoming = false
    var baseRegister: Int? = null
}

internal class InvokeAllocation(
    val opcode: Opcode,
    val ref: MethodRef,
    val registers: List<Register>,
) {
    var scratch: Register.Temporary? = null
    lateinit var operation: Op
}

internal typealias Emit = (InstructionBuilder, resolve: (Register) -> Int) -> Unit

internal class Op(
    val reads: List<Register> = emptyList(),
    val writes: List<Register> = emptyList(),
    val label: EmissionLabel? = null,
    val target: EmissionLabel? = null,
    val fallsThrough: Boolean = true,
    val emit: Emit,
)

internal data class Lifetime(val first: Int, val last: Int)

internal enum class Equality {
    EQUAL,
    NOT_EQUAL,
}

internal enum class EmissionMode {
    INSERTION,
    REPLACEMENT,
}
