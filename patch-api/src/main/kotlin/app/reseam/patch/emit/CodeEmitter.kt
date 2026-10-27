// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.Capture
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.buildInstructions
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.opcode
import app.reseam.patch.types.CodeInstruction
import app.reseam.patch.types.CodePlacement
import app.reseam.patch.types.CodeRegister
import app.reseam.patch.types.CodeStep
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction

@JvmInline internal value class Label(val id: UInt)

/**
 * The code a patch writes in a block, recorded over temporaries for the engine to place at
 * [placement]: the engine chooses their registers and the encodings that reach them.
 */
internal class CodeEmitter(
    internal val method: Method,
    internal val placement: CodePlacement,
    internal val captures: List<Capture>,
) {
    internal val steps = mutableListOf<CodeStep>()
    private var temporaries = 0u
    private var labels = 0u

    internal val info = method.info
    internal val access = Access(info.classDescriptor)
    internal val isStaticMethod = AccessFlags.STATIC.isSet(info.accessFlags)

    val code = EmittedCode(this)

    fun temporary(
        type: String,
        source: FieldRef? = null,
        assignableType: ValueType = ValueType.Known(type),
    ): Value = Value(this, CodeRegister.Temporary(temporaries++), type, source, assignableType)

    /** The incoming register word at [offset], holding a value of [type]. */
    fun parameter(offset: Int, type: String): Value =
        Value(this, CodeRegister.Parameter(offset.toUShort()), type)

    fun fixed(register: Int, type: String): Value =
        Value(this, CodeRegister.Fixed(register.toUShort()), type)

    fun label(): Label = Label(labels++)

    /**
     * Records [template], an instruction whose registers are placeholders for [operands] in
     * encoding order, or for its arguments when it is a call. A branch goes to [target].
     */
    fun emit(template: Instruction, vararg operands: Value, target: Label? = null) =
        emitAt(steps.size, template, *operands, target = target)

    fun emitAt(index: Int, template: Instruction, vararg operands: Value, target: Label? = null) {
        steps.add(
            index,
            CodeStep.Instruction(
                CodeInstruction(template, operands.map { it.operand }, target?.id)
            ),
        )
    }

    fun place(label: Label) {
        steps += CodeStep.Label(label.id)
    }
}

/** The one instruction [build] writes, as a template whose registers [CodeEmitter.emit] fills. */
internal fun template(build: InstructionBuilder.() -> Unit): Instruction =
    buildInstructions(build).single()

internal val CodeStep.fallsThrough: Boolean
    get() = this !is CodeStep.Instruction || field0.template.opcode?.endsFlow != true
