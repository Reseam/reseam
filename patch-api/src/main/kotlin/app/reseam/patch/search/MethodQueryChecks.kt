// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.PatchRuntime
import app.reseam.patch.descriptor
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.allSet
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType

internal fun MethodQuerySpec.rejection(value: Method, runtime: PatchRuntime): String? {
    val index = runtime.index
    val info = value.info
    // `inClass` names the owner, so the search never wandered into extension code and the
    // methods of an extension class stay reachable through its own target.
    if (inClass == null && isHiddenExtension(info.dexIndex.toInt(), index))
        return "defined by a bundle extension"
    methodName?.let { if (info.methodName != it) return "name mismatch" }
    if (stringValues.any { !index.seeds.hasString(value, it) }) return "missing required string"
    if (stringPrefixes.any { !index.seeds.hasStringPrefix(value, it) })
        return "missing required string prefix"
    if (literalValues.any { !index.seeds.hasLiteral(value, it) }) return "missing required literal"
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
    if (calls.any { ref -> !index.references.calls(value, ref().signature) })
        return "invoke target mismatch"
    if (
        calledBy.any { caller ->
            !index.references.calls(runtime.resolve(caller).value, value.signature)
        }
    )
        return "caller mismatch"
    if (callMatches.any { index.references.callsIn(value, it).isEmpty() }) return "no matching call"
    if (callPredicates.any { predicate -> index.references.methodRefsOf(value).none(predicate) })
        return "no matching call"
    if (accessMatches.any { index.references.accessesIn(value, it).isEmpty() })
        return "no matching field access"
    if (opcodes.any { !index.seeds.hasOpcode(value, it) }) return "opcode mismatch"
    if (opcodeSequences.any { value.indexOfOpcodeSequence(*it.toTypedArray()) == null })
        return "opcode sequence mismatch"
    if (predicates.any { !it(value) }) return "custom predicate rejected it"
    return null
}

internal fun MethodQuerySpec.reasons(): List<String> = buildList {
    methodName?.let { add("matched name $it") }
    if (stringValues.isNotEmpty()) add("matched strings ${stringValues.joinToString()}")
    if (stringPrefixes.isNotEmpty()) add("matched string prefixes ${stringPrefixes.joinToString()}")
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
    if (calls.isNotEmpty()) add("matched invoke targets ${calls.joinToString { it().descriptor }}")
    if (calledBy.isNotEmpty()) add("matched caller ${calledBy.joinToString { it.label }}")
    if (callMatches.isNotEmpty()) add("matched ${callMatches.size} indexed call query(s)")
    if (callPredicates.isNotEmpty()) add("matched ${callPredicates.size} call predicate(s)")
    if (accessMatches.isNotEmpty())
        add("matched ${accessMatches.size} indexed field access query(s)")
    if (opcodes.isNotEmpty()) add("matched opcodes ${opcodes.joinToString()}")
    opcodeSequences.forEach { add("matched opcode sequence ${describeSequence(it)}") }
    if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
    extensionReason()?.let(::add)
}
