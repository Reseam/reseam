// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.invokeArgumentTypes
import app.reseam.patch.dex.invokeRegisters
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.native.Instruction

internal data class InvokeArgument(val register: Int, val type: String)

/** One convention for captures, origins, conditional calls and redirection. */
internal fun Instruction.arguments(label: String): List<InvokeArgument> {
    val types = requireNotNull(invokeArgumentTypes) { "$label: instruction is not an invoke, so it passes no arguments" }
    val registers = invokeRegisters.orEmpty()
    require(types.sumOf(::registerWordCount) == registers.size) { "$label: malformed invoke argument word count" }
    var word = 0
    return types.map { type ->
        val register = registers[word]
        val width = registerWordCount(type)
        require(width != 2 || registers[word + 1] == register + 1) { "$label: nonconsecutive wide argument" }
        word += width
        InvokeArgument(register, type)
    }
}
