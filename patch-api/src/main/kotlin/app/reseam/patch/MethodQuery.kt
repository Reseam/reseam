// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.allSet
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.MethodRef

internal class MethodQuerySpec : QuerySpec<Method, MethodRankScope>("method"), MethodQuery {
    private var methodName: String? = null
    private val stringValues = mutableListOf<String>()
    private val stringPrefixes = mutableListOf<String>()
    private val literalValues = mutableListOf<Long>()
    private var returnType: String? = null
    private var parameterTypes: List<String>? = null
    private val positionalParams = mutableMapOf<Int, String>()
    private val hasParameters = mutableListOf<String>()
    private var parameterCount: Int? = null
    private var flagMask = 0
    private var inClass: ClassTarget? = null
    private var methodScope = MethodScope.DECLARED
    private val calls = mutableListOf<() -> MethodRef>()
    private val calledBy = mutableListOf<MethodTarget>()
    private val callMatches = mutableListOf<MethodRefMatchSpec>()
    private val callPredicates = mutableListOf<MethodRef.() -> Boolean>()
    private val opcodes = mutableListOf<Opcode>()
    private val opcodeSequences = mutableListOf<List<Opcode?>>()
    private val predicates = mutableListOf<Method.() -> Boolean>()

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
        val spec = MethodRefMatchSpec().apply(block)
        require(
            spec.owner != null ||
                spec.name != null ||
                spec.returnType != null ||
                spec.parameters != null
        ) {
            "calls requires an owner, name, return type, or parameter signature"
        }
        callMatches += spec
    }

    override fun calledBy(target: MethodTarget) {
        calledBy += target
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

    override fun candidates(runtime: PatchRuntime): CandidatePool<Method> {
        val index = runtime.index
        val seeds = mutableListOf<Pair<String, Set<UInt>>>()
        inClass?.let { target ->
            val owner = runtime.resolve(target).value.descriptor
            val declared = index.methodsInClass(owner, methodName)
            val inherited = index.inheritedMethods(owner, methodName)
            val label =
                when {
                    methodScope == MethodScope.INHERITED ->
                        "inClass($owner) with ${inherited.size} inherited"
                    inherited.isEmpty() -> "inClass($owner)"
                    else ->
                        "inClass($owner) declaring only; ${inherited.size} inherited method(s) need inClass(target, inherited = true)"
                }
            val candidates =
                if (methodScope == MethodScope.INHERITED) declared + inherited else declared
            seeds += label to candidates.map(Method::handle).toSet()
        }
        calls.forEach { resolve ->
            val ref = resolve()
            seeds +=
                "calls(${ref.descriptor})" to
                    index
                        .invokeSitesFor(MethodSignature(ref.definingClass, ref.name, ref.proto))
                        .map { it.first }
                        .toSet()
        }
        calledBy.forEach { target ->
            val method = runtime.resolve(target).value
            seeds +=
                "calledBy(${method.descriptor})" to
                    index.calleesOf(method).map(Method::handle).toSet()
        }
        if (stringValues.isNotEmpty()) {
            seeds +=
                "strings(${stringValues.joinToString(transform = ::quoted)})" to
                    index.methodsWithStrings(stringValues)
        }
        stringPrefixes.forEach {
            seeds += "stringsStartingWith(${quoted(it)})" to index.methodsWithStringPrefix(it)
        }
        literalValues.forEach { seeds += "literals($it)" to index.methodsWithLiteral(it) }
        if (seeds.isEmpty()) {
            callMatches.forEach { seeds += "calls(reference query)" to index.methodsCalling(it) }
        }
        if (seeds.isEmpty()) {
            methodName?.let { seeds += "name($it)" to index.methodsWithName(it) }
        }
        if (seeds.isEmpty()) {
            returnType?.let { seeds += "returns($it)" to index.methodsWithReturnType(it) }
            parameterTypes?.let {
                seeds += "params(${it.joinToString()})" to index.methodsWithExactParameters(it)
            }
            hasParameters.forEach { seeds += "hasParam($it)" to index.methodsWithParameter(it) }
            opcodes.forEach { seeds += "opcode($it)" to index.methodsWithOpcode(it) }
            opcodeSequences.forEach {
                seeds +=
                    "opcodeSequence(${describeSequence(it)})" to index.methodsWithOpcodeSequence(it)
            }
        }
        if (seeds.isEmpty()) {
            return CandidatePool(
                index.allMethods,
                listOf(
                    "no selective constraints; considering all ${index.allMethods.size} methods"
                ),
                index.allMethods.size,
                index.allMethods.take(12),
                null,
            )
        }
        return index.poolFromSeeds(seeds, "method(s)") { handles -> handles.map(::Method) }
    }

    override fun rankScope(index: SearchIndex, value: Method) = MethodRankScopeImpl(index, value)

    override fun describe(value: Method) = value.descriptor

    override fun mismatch(value: Method, runtime: PatchRuntime): String? {
        val index = runtime.index
        val info = value.info
        // `inClass` names the owner, so the search never wandered into extension code and the
        // methods of an extension class stay reachable through its own target.
        if (inClass == null && isHiddenExtension(info.dexIndex.toInt(), index))
            return "defined by a bundle extension"
        methodName?.let { if (info.methodName != it) return "name mismatch" }
        if (stringValues.any { !index.methodHasString(value, it) }) return "missing required string"
        if (stringPrefixes.any { value.handle !in index.methodsWithStringPrefix(it) })
            return "missing required string prefix"
        if (literalValues.any { !value.containsLiteral(it) }) return "missing required literal"
        returnType?.let { if (value.returnType != it) return "return type mismatch" }
        parameterTypes?.let { if (value.parameterTypes != it) return "parameter type mismatch" }
        val params = value.parameterTypes
        if (positionalParams.any { (i, type) -> params.getOrNull(i) != type })
            return "positional parameter mismatch"
        if (hasParameters.any { it !in params }) return "missing parameter"
        parameterCount?.let { if (params.size != it) return "parameter count mismatch" }
        if (!flagMask.allSet(info.accessFlags)) return "access flags mismatch"
        inClass?.let { target ->
            val owner = runtime.resolve(target).value.descriptor
            val declares = info.classDescriptor == owner
            if (
                !declares &&
                    !(methodScope == MethodScope.INHERITED &&
                        index.inheritedMethods(owner, methodName).any { it.handle == value.handle })
            ) {
                return "class mismatch"
            }
        }
        if (calls.any { ref -> !index.methodCalls(value, ref()) }) return "invoke target mismatch"
        if (calledBy.any { caller -> !index.methodCalls(runtime.resolve(caller).value, value) })
            return "caller mismatch"
        if (callMatches.any { !index.matchesCall(value, it) }) return "no matching call"
        if (callPredicates.any { predicate -> index.methodRefsOf(value).none(predicate) })
            return "no matching call"
        if (opcodes.any { !index.methodHasOpcode(value, it) }) return "opcode mismatch"
        if (opcodeSequences.any { value.indexOfOpcodeSequence(*it.toTypedArray()) == null })
            return "opcode sequence mismatch"
        if (predicates.any { !it(value) }) return "custom predicate rejected it"
        return null
    }

    override fun matchReasons(): List<String> = buildList {
        methodName?.let { add("matched name $it") }
        if (stringValues.isNotEmpty()) add("matched strings ${stringValues.joinToString()}")
        if (stringPrefixes.isNotEmpty())
            add("matched string prefixes ${stringPrefixes.joinToString()}")
        if (literalValues.isNotEmpty()) add("matched literals ${literalValues.joinToString()}")
        returnType?.let { add("matched return type $it") }
        parameterTypes?.let { add("matched parameter types ${it.joinToString()}") }
        if (positionalParams.isNotEmpty())
            add(
                "matched parameters ${positionalParams.entries.joinToString { "${it.key}:${it.value}" }}"
            )
        if (hasParameters.isNotEmpty())
            add("matched parameter contains ${hasParameters.joinToString()}")
        parameterCount?.let { add("matched parameter count $it") }
        if (flagMask != 0) add("matched access flags 0x${flagMask.toString(16)}")
        inClass?.let { add("matched class ${it.label}") }
        if (calls.isNotEmpty())
            add("matched invoke targets ${calls.joinToString { it().descriptor }}")
        if (calledBy.isNotEmpty()) add("matched caller ${calledBy.joinToString { it.label }}")
        if (callMatches.isNotEmpty()) add("matched ${callMatches.size} indexed call query(s)")
        if (callPredicates.isNotEmpty()) add("matched ${callPredicates.size} call predicate(s)")
        if (opcodes.isNotEmpty()) add("matched opcodes ${opcodes.joinToString()}")
        opcodeSequences.forEach { add("matched opcode sequence ${describeSequence(it)}") }
        if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
        extensionReason()?.let(::add)
    }
}
