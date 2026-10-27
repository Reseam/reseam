// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.countFollowingCheckCast
import app.reseam.patch.native.fieldReferences
import app.reseam.patch.native.findCallsMatching
import app.reseam.patch.native.findFieldAccessesMatching
import app.reseam.patch.native.instructionMethodRef
import app.reseam.patch.native.methodReferences
import app.reseam.patch.run.Memos
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.InstructionHit
import app.reseam.patch.types.MethodRef

private data class CastQuery(val callee: MethodSignature, val type: String, val lookAhead: Int)

/** The methods and fields each method's code refers to, and the code referring to each. */
internal class References(memos: Memos, private val index: SearchIndex) {
    private val methodRefs = memos.perMethod<List<MethodRef>>()
    private val fieldRefs = memos.perMethod<List<FieldRef>>()
    private val invokeSites = memos.clearedOnEdit<MethodSignature, List<Pair<UInt, Int>>>()
    private val castCounts = memos.clearedOnEdit<CastQuery, Int>()
    private val matchingCalls = memos.clearedOnEdit<MethodRefMatchSpec, Set<UInt>>()
    private val matchingAccesses = memos.clearedOnEdit<FieldRefMatchSpec, Set<UInt>>()

    fun methodRefsOf(method: Method): List<MethodRef> =
        methodRefs.getOrPut(method.handle) { methodReferences(method.handle).distinct() }

    fun fieldRefsOf(method: Method): List<FieldRef> =
        fieldRefs.getOrPut(method.handle) { fieldReferences(method.handle) }

    fun calleesOf(method: Method): List<Method> = methodRefsOf(method).mapNotNull(index::methodFor)

    fun calls(method: Method, callee: MethodSignature): Boolean =
        invokeSites[callee]?.any { it.first == method.handle }
            ?: methodRefsOf(method).any { it.signature == callee }

    fun matchesCall(method: Method, spec: MethodRefMatchSpec): Boolean =
        matchingCalls[spec]?.contains(method.handle) ?: methodRefsOf(method).any(spec::matches)

    fun methodsCalling(spec: MethodRefMatchSpec): Set<UInt> =
        matchingCalls.getOrPut(spec) {
            findCallsMatching(
                    spec.owner,
                    spec.name,
                    spec.returnType,
                    spec.parameters,
                    spec.requiredParameters,
                    spec.parameterCount?.toUInt(),
                )
                .filter { hit ->
                    !spec.needsPostFilter ||
                        instructionMethodRef(hit.method, hit.index)?.let(spec::matches) == true
                }
                .mapTo(linkedSetOf()) { it.method }
        }

    fun matchesAccess(method: Method, spec: FieldRefMatchSpec): Boolean =
        matchingAccesses[spec]?.contains(method.handle) ?: fieldRefsOf(method).any(spec::matches)

    fun methodsAccessing(spec: FieldRefMatchSpec): Set<UInt> =
        matchingAccesses.getOrPut(spec) {
            findFieldAccessesMatching(spec.owner, spec.name, spec.fieldType).mapTo(linkedSetOf()) {
                it.method
            }
        }

    /** The methods invoking [callee], with the index of each invoke. */
    fun invokeSites(callee: MethodSignature): List<Pair<UInt, Int>> =
        invokeSites.getOrPut(callee) {
            val ref = MethodRef(callee.owner, callee.name, callee.proto)
            findCallsMatching(
                    ref.definingClass,
                    ref.name,
                    ref.returnType,
                    ref.parameterTypes,
                    emptyList(),
                    null,
                )
                .map { it.method to it.index.toInt() }
        }

    fun followedByCheckCast(callee: Method, type: String, lookAhead: Int): Int {
        val signature = callee.signature
        return castCounts.getOrPut(CastQuery(signature, type, lookAhead)) {
            countFollowingCheckCast(
                    invokeSites(signature).map { (handle, index) ->
                        InstructionHit(handle, index.toUInt())
                    },
                    type,
                    lookAhead.coerceAtLeast(0).toUInt(),
                )
                .toInt()
        }
    }
}
