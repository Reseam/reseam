// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.Type
import app.reseam.patch.types.Instruction

internal enum class RegisterAccess {
    READ,
    WRITE,
    READ_WRITE,
    REFINE;

    val reads: Boolean
        get() = this != WRITE

    val defines: Boolean
        get() = this != READ
}

internal data class RegisterUse(val words: Int, val access: RegisterAccess)

internal val arrayReads = Opcode.AGET..Opcode.AGET_SHORT

internal val arrayWrites = Opcode.APUT..Opcode.APUT_SHORT

internal val instanceFieldReads = Opcode.IGET..Opcode.IGET_SHORT

internal val instanceFieldWrites = Opcode.IPUT..Opcode.IPUT_SHORT

internal val staticFieldWrites = Opcode.SPUT..Opcode.SPUT_SHORT

internal val comparisons = Opcode.IF_EQ..Opcode.IF_LE

internal val zeroTests = Opcode.IF_EQZ..Opcode.IF_LEZ

internal val valueReturns = setOf(Opcode.RETURN, Opcode.RETURN_WIDE, Opcode.RETURN_OBJECT)

internal sealed interface OperandType {
    data class Stated(val descriptor: String) : OperandType

    data object Written : OperandType

    data object Array : OperandType

    data object Element : OperandType

    data object Returned : OperandType
}

internal data class Operand(val register: Int, val words: Int, val type: OperandType)

/**
 * The values an instruction takes the way a call takes arguments: an invoke's arguments, receiver
 * first, an array read as `get(array, index)` and a write as `set(array, index, value)`, an
 * instance field read as `get(object)` and a write as `set(object, value)`, a static field write as
 * `set(value)`, a branch as `test(value)` or `test(left, right)`, a return as `return(value)`, and
 * arithmetic, conversions and numeric comparisons as `op(left, right)` or `op(value)`. Null for any
 * other instruction.
 */
internal fun Instruction.operands(label: String): List<Operand>? {
    val opcode = opcode ?: return null
    if (invokeArgumentTypes != null) {
        return arguments(label).map {
            Operand(it.register, registerWordCount(it.type), OperandType.Stated(it.type))
        }
    }
    val reads = opcode.registers.zip(referencedRegisters).filter { (use, _) -> use.access.reads }
    val field = fieldRef
    val types: List<OperandType> =
        when {
            opcode in arrayReads -> listOf(OperandType.Array, OperandType.Stated(Type.Int))
            opcode in arrayWrites ->
                listOf(OperandType.Element, OperandType.Array, OperandType.Stated(Type.Int))
            opcode in instanceFieldReads -> listOf(OperandType.Stated(field!!.definingClass))
            opcode in instanceFieldWrites ->
                listOf(
                    OperandType.Stated(field!!.fieldType),
                    OperandType.Stated(field.definingClass),
                )
            opcode in staticFieldWrites -> listOf(OperandType.Stated(field!!.fieldType))
            opcode in zeroTests || opcode in comparisons -> reads.map { OperandType.Written }
            opcode in valueReturns -> listOf(OperandType.Returned)
            else -> opcode.arithmetic?.operands?.map(OperandType::Stated) ?: return null
        }
    check(types.size == reads.size) { "$label: $opcode reads ${reads.size} registers" }
    val operands = reads.zip(types) { (use, register), type -> Operand(register, use.words, type) }
    // A store's encoding names the stored value first; as a call it comes last.
    return if (opcode in arrayWrites || opcode in instanceFieldWrites) {
        operands.drop(1) + operands.first()
    } else operands
}

internal class Arithmetic(val operands: List<String>, val result: String)

internal val Opcode.arithmetic: Arithmetic?
    get() =
        when (this) {
            in Opcode.ADD_INT..Opcode.USHR_INT,
            in Opcode.ADD_INT_2ADDR..Opcode.USHR_INT_2ADDR -> binary(Type.Int)
            in Opcode.ADD_LONG..Opcode.XOR_LONG,
            in Opcode.ADD_LONG_2ADDR..Opcode.XOR_LONG_2ADDR -> binary(Type.Long)
            in Opcode.SHL_LONG..Opcode.USHR_LONG,
            in Opcode.SHL_LONG_2ADDR..Opcode.USHR_LONG_2ADDR ->
                Arithmetic(listOf(Type.Long, Type.Int), Type.Long)
            in Opcode.ADD_FLOAT..Opcode.REM_FLOAT,
            in Opcode.ADD_FLOAT_2ADDR..Opcode.REM_FLOAT_2ADDR -> binary(Type.Float)
            in Opcode.ADD_DOUBLE..Opcode.REM_DOUBLE,
            in Opcode.ADD_DOUBLE_2ADDR..Opcode.REM_DOUBLE_2ADDR -> binary(Type.Double)
            in Opcode.ADD_INT_LIT16..Opcode.USHR_INT_LIT8 -> unary(Type.Int, Type.Int)
            Opcode.CMPL_FLOAT,
            Opcode.CMPG_FLOAT -> Arithmetic(listOf(Type.Float, Type.Float), Type.Int)
            Opcode.CMPL_DOUBLE,
            Opcode.CMPG_DOUBLE -> Arithmetic(listOf(Type.Double, Type.Double), Type.Int)
            Opcode.CMP_LONG -> Arithmetic(listOf(Type.Long, Type.Long), Type.Int)
            Opcode.NEG_INT,
            Opcode.NOT_INT -> unary(Type.Int, Type.Int)
            Opcode.NEG_LONG,
            Opcode.NOT_LONG -> unary(Type.Long, Type.Long)
            Opcode.NEG_FLOAT -> unary(Type.Float, Type.Float)
            Opcode.NEG_DOUBLE -> unary(Type.Double, Type.Double)
            Opcode.INT_TO_LONG -> unary(Type.Int, Type.Long)
            Opcode.INT_TO_FLOAT -> unary(Type.Int, Type.Float)
            Opcode.INT_TO_DOUBLE -> unary(Type.Int, Type.Double)
            Opcode.LONG_TO_INT -> unary(Type.Long, Type.Int)
            Opcode.LONG_TO_FLOAT -> unary(Type.Long, Type.Float)
            Opcode.LONG_TO_DOUBLE -> unary(Type.Long, Type.Double)
            Opcode.FLOAT_TO_INT -> unary(Type.Float, Type.Int)
            Opcode.FLOAT_TO_LONG -> unary(Type.Float, Type.Long)
            Opcode.FLOAT_TO_DOUBLE -> unary(Type.Float, Type.Double)
            Opcode.DOUBLE_TO_INT -> unary(Type.Double, Type.Int)
            Opcode.DOUBLE_TO_LONG -> unary(Type.Double, Type.Long)
            Opcode.DOUBLE_TO_FLOAT -> unary(Type.Double, Type.Float)
            Opcode.INT_TO_BYTE -> unary(Type.Int, Type.Byte)
            Opcode.INT_TO_CHAR -> unary(Type.Int, Type.Char)
            Opcode.INT_TO_SHORT -> unary(Type.Int, Type.Short)
            else -> null
        }

private fun binary(type: String) = Arithmetic(listOf(type, type), type)

private fun unary(operand: String, result: String) = Arithmetic(listOf(operand), result)
