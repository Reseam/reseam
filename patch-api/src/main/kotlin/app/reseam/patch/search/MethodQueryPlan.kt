// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.PatchRuntime
import app.reseam.patch.descriptor
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType

/**
 * Seeds from the first group that has any. A seed costs a handle for every method it holds for the
 * rest of the run, so a group a query has no need of is never computed.
 */
internal fun MethodQuerySpec.seedPlan(runtime: PatchRuntime): QueryPlan<Method> {
    val index = runtime.index
    val owner = inClass?.let { runtime.resolve(it).value.descriptor }
    val callTargets = calls.map { it() }
    val callers = calledBy.map { runtime.resolve(it).value }
    val selective =
        buildList<() -> Seeded> {
            owner?.let { owner -> add { classSeed(index, owner) } }
            callers.forEach { caller ->
                add {
                    Seeded(
                        "calledBy(${caller.descriptor})",
                        index.references.calleesOf(caller).map(Method::handle).toSet(),
                    )
                }
            }
            if (stringValues.isNotEmpty()) {
                add {
                    Seeded(
                        "strings(${stringValues.joinToString(transform = ::quoted)})",
                        index.seeds.withStrings(stringValues),
                    )
                }
            }
            callTargets.forEach { ref ->
                add {
                    Seeded(
                        "calls(${ref.descriptor})",
                        index.references.invokeSites(ref.signature).map { it.first }.toSet(),
                    )
                }
            }
            stringPrefixes.forEach {
                add {
                    Seeded(
                        "stringsStartingWith(${quoted(it)})",
                        index.seeds.withStringPrefix(it),
                    )
                }
            }
            stringParts.forEach {
                add { Seeded("stringsContaining(${quoted(it)})", index.seeds.withStringPart(it)) }
            }
            literalValues.forEach {
                add { Seeded("literals($it)", index.seeds.withLiteral(it)) }
            }
        }
    val references =
        callMatches.map {
            { Seeded("calls(reference query)", index.references.methodsCalling(it)) }
        } +
            accessMatches.map {
                { Seeded("accesses(reference query)", index.references.methodsAccessing(it)) }
            }
    val name = listOfNotNull(methodName?.let { { Seeded("name($it)", index.seeds.named(it)) } })
    val seeds =
        selective
            .ifEmpty { references }
            .ifEmpty { name }
            .ifEmpty { declarationSeeds(index) + codeSeeds(index) }
    return QueryPlan(seeds, "method(s)", { handles -> handles.map(::Method) }) {
        CandidatePool(
            index.allMethods,
            listOf("no selective constraints; considering all ${index.allMethods.size} methods"),
            index.allMethods.size,
            index.allMethods.take(12),
            null,
        )
    }
}

private fun MethodQuerySpec.classSeed(index: SearchIndex, owner: String): Seeded {
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
    val candidates = if (methodScope == MethodScope.INHERITED) declared + inherited else declared
    return Seeded(label, candidates.map(Method::handle).toSet())
}

private fun MethodQuerySpec.declarationSeeds(index: SearchIndex): List<() -> Seeded> = buildList {
    parameterTypes?.let { types ->
        add {
            Seeded("params(${types.joinToString()})", index.seeds.withParameters(types))
        }
    }
    returnType?.let { type ->
        add { Seeded("returns($type)", index.seeds.returning(type)) }
    }
    hasParameters.forEach { type ->
        add { Seeded("hasParam($type)", index.seeds.withParameter(type)) }
    }
    positionalParams.forEach { (position, type) ->
        add { Seeded("param($position, $type)", index.seeds.withParameter(type)) }
    }
}

private fun MethodQuerySpec.codeSeeds(index: SearchIndex): List<() -> Seeded> = buildList {
    opcodeSequences.forEach { sequence ->
        add {
            Seeded(
                "opcodeSequence(${describeSequence(sequence)})",
                index.seeds.withOpcodeSequence(sequence),
            )
        }
    }
    opcodes.forEach { opcode ->
        add { Seeded("opcode($opcode)", index.seeds.withOpcode(opcode)) }
    }
}
