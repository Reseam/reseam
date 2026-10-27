// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.native.findInstructionsByLiteral
import app.reseam.patch.native.findMethodsByName
import app.reseam.patch.native.findMethodsByOpcodes
import app.reseam.patch.native.findMethodsByProto
import app.reseam.patch.native.findMethodsByStringPrefix
import app.reseam.patch.native.findMethodsByStrings
import app.reseam.patch.native.indexOfFirstStringWithPrefix
import app.reseam.patch.run.Memos

private sealed interface Seed {
    data class Name(val value: String) : Seed

    data class Returns(val type: String) : Seed

    data class Parameters(val types: List<String>) : Seed

    data class Parameter(val type: String) : Seed

    data class Op(val value: Opcode) : Seed

    data class Ops(val values: List<Opcode?>) : Seed

    data class Strings(val values: List<String>) : Seed

    data class Prefix(val value: String) : Seed

    data class Literal(val value: Long) : Seed
}

/**
 * The methods holding a declaration or a piece of code, found by one scan of the app each and kept
 * for the run; a query seeds its candidates from them.
 */
internal class MethodSeeds(memos: Memos) {
    private val declarations = memos.keptOnEdit<Seed, Set<UInt>>()
    private val code = memos.clearedOnEdit<Seed, Set<UInt>>()

    private fun seed(memo: Memos.Memo<Seed, Set<UInt>>, key: Seed, find: () -> UIntArray) =
        memo.getOrPut(key) { find().toCollection(linkedSetOf()) }

    fun named(name: String) = seed(declarations, Seed.Name(name)) { findMethodsByName(name) }

    fun returning(type: String) =
        seed(declarations, Seed.Returns(type)) { findMethodsByProto(type, null, null) }

    fun withParameters(types: List<String>) =
        seed(declarations, Seed.Parameters(types.toList())) {
            findMethodsByProto(null, types, null)
        }

    fun withParameter(type: String) =
        seed(declarations, Seed.Parameter(type)) { findMethodsByProto(null, null, type) }

    fun withOpcode(opcode: Opcode) =
        seed(code, Seed.Op(opcode)) { findMethodsByOpcodes(intArrayOf(opcode.value)) }

    fun withOpcodeSequence(opcodes: List<Opcode?>) =
        seed(code, Seed.Ops(opcodes.toList())) { findMethodsByOpcodes(opcodes.toPattern()) }

    fun withStrings(values: List<String>): Set<UInt> {
        val key = values.distinct().sorted()
        return seed(code, Seed.Strings(key)) { findMethodsByStrings(key) }
    }

    fun withStringPrefix(prefix: String) =
        seed(code, Seed.Prefix(prefix)) { findMethodsByStringPrefix(prefix) }

    fun withLiteral(value: Long) =
        seed(code, Seed.Literal(value)) {
            findInstructionsByLiteral(value).map { it.method }.toUIntArray()
        }

    fun hasString(method: Method, value: String): Boolean = method.indexOfFirstString(value) != null

    fun hasOpcode(method: Method, opcode: Opcode): Boolean = method.indexOfFirst(opcode) != null

    fun hasStringPrefix(method: Method, prefix: String): Boolean =
        code[Seed.Prefix(prefix)]?.contains(method.handle)
            ?: (indexOfFirstStringWithPrefix(method.handle, prefix) != null)

    fun hasLiteral(method: Method, value: Long): Boolean =
        code[Seed.Literal(value)]?.contains(method.handle) ?: method.containsLiteral(value)
}
