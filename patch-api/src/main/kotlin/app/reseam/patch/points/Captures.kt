// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.points

import app.reseam.patch.Type
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.invokeRegisters
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.referencedRegisters
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.typeRef
import app.reseam.patch.types.Instruction

internal fun inferType(method: Method, index: Int): String? {
    val insns = method.instructions
    val insn = insns[index]
    return when (insn.opcode) {
        Opcode.CHECK_CAST,
        Opcode.NEW_INSTANCE,
        Opcode.NEW_ARRAY -> insn.typeRef
        Opcode.CONST_STRING,
        Opcode.CONST_STRING_JUMBO -> Type.String
        Opcode.CONST_CLASS -> "Ljava/lang/Class;"
        Opcode.INSTANCE_OF,
        Opcode.ARRAY_LENGTH -> Type.Int
        Opcode.MOVE_RESULT,
        Opcode.MOVE_RESULT_WIDE,
        Opcode.MOVE_RESULT_OBJECT -> insns.getOrNull(index - 1)?.methodRef?.returnType
        Opcode.MOVE_EXCEPTION -> "Ljava/lang/Throwable;"
        Opcode.IGET,
        Opcode.IGET_WIDE,
        Opcode.IGET_OBJECT,
        Opcode.IGET_BOOLEAN,
        Opcode.IGET_BYTE,
        Opcode.IGET_CHAR,
        Opcode.IGET_SHORT,
        Opcode.SGET,
        Opcode.SGET_WIDE,
        Opcode.SGET_OBJECT,
        Opcode.SGET_BOOLEAN,
        Opcode.SGET_BYTE,
        Opcode.SGET_CHAR,
        Opcode.SGET_SHORT -> insn.fieldRef?.fieldType
        else -> null
    }
}

internal fun writerMessage(
    label: String,
    method: Method,
    index: Int,
    register: Int,
    writers: ArgumentOrigins.Origins,
): String {
    val where = "v$register at ${method.descriptor}[$index]"
    val sites = writers.sites.joinToString { "[$it] ${method.instructions[it].opcode}" }
    return when {
        writers.unknown ->
            "$label: $where has unknown control flow, cyclic copies or inconsistent wide sources"
        writers.sites.isEmpty() && writers.entry ->
            "$label: $where is a value the method was passed, not one an instruction of it wrote"
        writers.sites.isEmpty() -> "$label: nothing reaches $where; the instruction is unreachable"
        writers.entry ->
            "$label: $where is either what the method was passed or one of $sites, depending on the path"
        else -> "$label: $where is written on more than one path in: $sites"
    }
}

internal fun noRegisterMessage(label: String, index: Int, insn: Instruction): String =
    when {
        insn.invokeRegisters != null ->
            "$label: instruction $index (${insn.opcode}) writes no register; capture one of its arguments with captureArgumentAs(name, argument)"
        insn.referencedRegisters.isEmpty() ->
            "$label: instruction $index (${insn.opcode}) uses no register at all, so there is nothing to capture"
        else -> "$label: instruction $index (${insn.opcode}) writes no register to capture"
    }
