// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.ClassTarget
import app.reseam.patch.FieldRefMatch
import app.reseam.patch.MethodQuery
import app.reseam.patch.MethodRankScope
import app.reseam.patch.MethodRefMatch
import app.reseam.patch.MethodTarget
import app.reseam.patch.PatchRuntime
import app.reseam.patch.descriptor
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.FieldPattern
import app.reseam.patch.types.MethodPattern
import app.reseam.patch.types.MethodRef

internal enum class MethodScope {
    DECLARED,
    INHERITED,
}

internal class MethodQuerySpec : QuerySpec<Method, MethodRankScope>("method"), MethodQuery {
    var methodName: String? = null
        private set

    val stringValues = mutableListOf<String>()
    val stringPrefixes = mutableListOf<String>()
    val literalValues = mutableListOf<Long>()
    var returnType: String? = null
        private set

    var parameterTypes: List<String>? = null
        private set

    val positionalParams = mutableMapOf<Int, String>()
    val hasParameters = mutableListOf<String>()
    var parameterCount: Int? = null
        private set

    var flagMask = 0
        private set

    var inClass: ClassTarget? = null
        private set

    var methodScope = MethodScope.DECLARED
        private set

    val calls = mutableListOf<() -> MethodRef>()
    val calledBy = mutableListOf<MethodTarget>()
    val callMatches = mutableListOf<MethodPattern>()
    val callPredicates = mutableListOf<MethodRef.() -> Boolean>()
    val accessMatches = mutableListOf<FieldPattern>()
    val opcodes = mutableListOf<Opcode>()
    val opcodeSequences = mutableListOf<List<Opcode?>>()
    val predicates = mutableListOf<Method.() -> Boolean>()

    override fun name(value: String) {
        methodName = value
    }

    override fun strings(vararg values: String) {
        stringValues += values
    }

    override fun stringsStartingWith(prefix: String) {
        require(prefix.isNotEmpty()) { "A string prefix must not be empty" }
        stringPrefixes += prefix
    }

    override fun literals(vararg values: Long) {
        literalValues += values.toList()
    }

    override fun returns(type: String) {
        returnType = descriptor(type)
    }

    override fun params(vararg types: String) {
        parameterTypes = types.map(::descriptor)
    }

    override fun param(index: Int, type: String) {
        positionalParams[index] = descriptor(type)
    }

    override fun hasParam(type: String) {
        hasParameters += descriptor(type)
    }

    override fun paramCount(count: Int) {
        parameterCount = count
    }

    override fun flags(mask: Int) {
        flagMask = flagMask or mask
    }

    override fun inClass(target: ClassTarget, inherited: Boolean) {
        inClass = target
        methodScope = if (inherited) MethodScope.INHERITED else MethodScope.DECLARED
    }

    override fun calls(target: MethodTarget) {
        calls += { target.ref }
    }

    override fun calls(ref: MethodRef) {
        calls += { ref }
    }

    override fun calls(block: MethodRefMatch.() -> Unit) {
        val pattern = MethodRefMatchSpec().apply(block).pattern
        require(
            pattern.owner != null ||
                pattern.name != null ||
                pattern.returnType != null ||
                pattern.parameters != null
        ) {
            "calls requires an owner, name, return type, or parameter signature"
        }
        callMatches += pattern
    }

    override fun calledBy(target: MethodTarget) {
        calledBy += target
    }

    override fun accesses(block: FieldRefMatch.() -> Unit) {
        val pattern = FieldRefMatchSpec().apply(block).pattern
        require(pattern.owner != null || pattern.name != null || pattern.fieldType != null) {
            "accesses requires an owner, name, or type"
        }
        accessMatches += pattern
    }

    override fun callsMethod(predicate: MethodRef.() -> Boolean) {
        callPredicates += predicate
    }

    override fun opcode(vararg opcodes: Opcode) {
        this.opcodes += opcodes
    }

    override fun opcodeSequence(vararg opcodes: Opcode?) {
        opcodeSequences += opcodes.toList()
    }

    override fun custom(predicate: Method.() -> Boolean) {
        predicates += predicate
    }

    override fun plan(runtime: PatchRuntime): QueryPlan<Method> = seedPlan(runtime)

    override fun prefetch(runtime: PatchRuntime, candidates: List<Method>) =
        runtime.run.cache.prefetchMethods(candidates.map { it.handle })

    override fun rankScope(index: SearchIndex, value: Method) = MethodRankScopeImpl(index, value)

    override fun describe(value: Method) = value.descriptor

    override fun mismatch(value: Method, runtime: PatchRuntime): String? = rejection(value, runtime)

    override fun matchReasons(): List<String> = reasons()
}
