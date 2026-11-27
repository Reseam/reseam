// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.dex

import app.reseam.patch.Type
import app.reseam.patch.dex.OperandType.Array
import app.reseam.patch.dex.OperandType.Element
import app.reseam.patch.dex.OperandType.Returned
import app.reseam.patch.dex.OperandType.Stated
import app.reseam.patch.dex.OperandType.Written
import app.reseam.patch.types.FieldRef
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class OperandsTest {
    private val count = FieldRef("Lcom/example/Host;", "count", Type.Long)

    @Test
    fun `instructions take their values the way a call takes arguments`() {
        val cases =
            listOf(
                instruction { aput(3, 4, 5, Type.Int) } to
                    listOf(
                        Operand(4, 1, Array),
                        Operand(5, 1, Stated(Type.Int)),
                        Operand(3, 1, Element),
                    ),
                instruction { aget(3, 4, 5, Type.Long) } to
                    listOf(Operand(4, 1, Array), Operand(5, 1, Stated(Type.Int))),
                instruction { iput(2, 7, count) } to
                    listOf(
                        Operand(7, 1, Stated("Lcom/example/Host;")),
                        Operand(2, 2, Stated(Type.Long)),
                    ),
                instruction { iget(2, 7, count) } to
                    listOf(Operand(7, 1, Stated("Lcom/example/Host;"))),
                instruction { sput(2, count) } to listOf(Operand(2, 2, Stated(Type.Long))),
                instruction { ifEqz(6, 4) } to listOf(Operand(6, 1, Written)),
                instruction { ifLt(6, 8, 4) } to
                    listOf(Operand(6, 1, Written), Operand(8, 1, Written)),
                instruction { returnWide(4) } to listOf(Operand(4, 2, Returned)),
                instruction { reg3(Opcode.SHL_LONG, 0, 2, 4) } to
                    listOf(Operand(2, 2, Stated(Type.Long)), Operand(4, 1, Stated(Type.Int))),
                instruction { reg2(Opcode.ADD_DOUBLE_2ADDR, 2, 4) } to
                    listOf(Operand(2, 2, Stated(Type.Double)), Operand(4, 2, Stated(Type.Double))),
                instruction { literal(Opcode.AND_INT_LIT16, 0, 0x1000, src = 3) } to
                    listOf(Operand(3, 1, Stated(Type.Int))),
                instruction { reg2(Opcode.LONG_TO_FLOAT, 0, 2) } to
                    listOf(Operand(2, 2, Stated(Type.Long))),
                instruction { cmpLong(0, 2, 4) } to
                    listOf(Operand(2, 2, Stated(Type.Long)), Operand(4, 2, Stated(Type.Long))),
                instruction {
                    invokeVirtual("Lcom/example/Host;", "put", "(JLjava/lang/String;)V", 1, 2, 3, 4)
                } to
                    listOf(
                        Operand(1, 1, Stated("Lcom/example/Host;")),
                        Operand(2, 2, Stated(Type.Long)),
                        Operand(4, 1, Stated(Type.String)),
                    ),
            )
        for ((instruction, expected) in cases) {
            assertEquals(expected, instruction.operands("test"), "${instruction.opcode}")
        }
    }

    @Test
    fun `instructions that take nothing the way a call does have no operands`() {
        for (instruction in
            listOf(instruction { constInt(0, 1) }, instruction { moveResult(0, Type.Int) })) {
            assertNull(instruction.operands("test"))
        }
    }

    @Test
    fun `arithmetic writes the type its operation produces`() {
        assertEquals(Type.Float, Opcode.LONG_TO_FLOAT.arithmetic?.result)
        assertEquals(Type.Int, Opcode.CMPG_DOUBLE.arithmetic?.result)
        assertEquals(Type.Long, Opcode.USHR_LONG_2ADDR.arithmetic?.result)
        assertEquals(Type.Char, Opcode.INT_TO_CHAR.arithmetic?.result)
        assertNull(Opcode.MOVE.arithmetic)
    }

    private fun instruction(build: InstructionBuilder.() -> Unit) =
        buildInstructions(build).single()
}
