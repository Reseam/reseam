// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodRef

internal val Instruction.opcodeValue: Int
    get() =
        when (this) {
            is Instruction.Simple -> field0.opcode.toInt()
            is Instruction.Reg1 -> field0.opcode.toInt()
            is Instruction.Reg2 -> field0.opcode.toInt()
            is Instruction.Reg3 -> field0.opcode.toInt()
            is Instruction.RegLiteral -> field0.opcode.toInt()
            is Instruction.RegString -> field0.opcode.toInt()
            is Instruction.RegType -> field0.opcode.toInt()
            is Instruction.RegField -> field0.opcode.toInt()
            is Instruction.Invoke -> field0.opcode.toInt()
            is Instruction.InvokeRange -> field0.opcode.toInt()
            is Instruction.Polymorphic -> field0.opcode.toInt()
            is Instruction.PolymorphicRange -> field0.opcode.toInt()
            is Instruction.Custom -> field0.opcode.toInt()
            is Instruction.CustomRange -> field0.opcode.toInt()
            is Instruction.RegHandle -> field0.opcode.toInt()
            is Instruction.RegProto -> field0.opcode.toInt()
            is Instruction.Branch0 -> field0.opcode.toInt()
            is Instruction.Branch -> field0.opcode.toInt()
            is Instruction.Branch2 -> field0.opcode.toInt()
            is Instruction.FilledArray -> field0.opcode.toInt()
            is Instruction.FilledArrayRange -> field0.opcode.toInt()
            is Instruction.PackedSwitchData,
            is Instruction.SparseSwitchData,
            is Instruction.FillArrayData -> -1
            is Instruction.Raw -> field0.firstOrNull()?.toInt()?.and(0xFF) ?: -1
        }

val Instruction.opcode: Opcode?
    get() = Opcode.of(opcodeValue)

val Instruction.methodRef: MethodRef?
    get() =
        when (this) {
            is Instruction.Invoke -> field0.method
            is Instruction.InvokeRange -> field0.method
            is Instruction.Polymorphic -> field0.method
            is Instruction.PolymorphicRange -> field0.method
            else -> null
        }

val Instruction.fieldRef: FieldRef?
    get() = (this as? Instruction.RegField)?.field0?.field

val Instruction.stringValue: String?
    get() = (this as? Instruction.RegString)?.field0?.value

val Instruction.typeRef: String?
    get() =
        when (this) {
            is Instruction.RegType -> field0.typeDescriptor
            is Instruction.FilledArray -> field0.typeDescriptor
            is Instruction.FilledArrayRange -> field0.typeDescriptor
            else -> null
        }

val Instruction.literal: Long?
    get() = (this as? Instruction.RegLiteral)?.field0?.literal
