// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.Opcode
import app.reseam.patch.types.Instruction

interface PointMatch {
    fun opcode(vararg opcodes: Opcode)

    fun string(value: String)

    fun stringContains(part: String)

    fun literal(value: Long)

    fun type(descriptor: String)

    fun checkCast(type: String)

    fun newInstance(type: String)

    fun invoke(vararg opcodes: Opcode, block: MethodRefMatch.() -> Unit = {})

    fun invokeStatic(block: MethodRefMatch.() -> Unit = {})

    fun invokeVirtual(block: MethodRefMatch.() -> Unit = {})

    fun invokeInterface(block: MethodRefMatch.() -> Unit = {})

    fun invokeDirect(block: MethodRefMatch.() -> Unit = {})

    /** An invoke of exactly this method. */
    fun calls(target: MethodTarget)

    fun field(block: FieldRefMatch.() -> Unit)

    /**
     * Every reaching source of an invoke argument must match; follows copies and exception paths.
     */
    fun argument(index: Int, block: PointMatch.() -> Unit)

    /** A `move-result` whose invoke returns `type`, or any when null. */
    fun resultOf(returns: String? = null)

    fun where(predicate: Instruction.() -> Boolean)

    /** The next step of the sequence, at most `within` instructions later. */
    fun then(within: Int = 1, block: PointMatch.() -> Unit)
}

interface MethodRefMatch {
    fun owner(type: String)

    /** The declared owner is this type or a known subtype of it. */
    fun ownerAssignableTo(type: String)

    fun name(value: String)

    fun returns(type: String)

    fun params(vararg types: String)

    fun hasParam(type: String)

    fun paramCount(count: Int)
}

interface FieldRefMatch {
    fun owner(type: String)

    fun name(value: String)

    fun type(descriptor: String)
}
