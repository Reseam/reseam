// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.dex.Method
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.countFollowingCheckCast
import app.reseam.patch.native.findCallsIn
import app.reseam.patch.native.findCallsMatching
import app.reseam.patch.native.findFieldAccessesIn
import app.reseam.patch.native.findFieldAccessesMatching
import app.reseam.patch.native.methodReferences
import app.reseam.patch.run.Memos
import app.reseam.patch.types.FieldPattern
import app.reseam.patch.types.InstructionHit
import app.reseam.patch.types.MethodPattern
import app.reseam.patch.types.MethodRef

private data class CastQuery(val callee: MethodSignature, val type: String, val lookAhead: Int)

/** The methods and fields each method's code refers to, and the code referring to each. */
internal class References(memos: Memos, private val index: SearchIndex) {
    private val methodRefs = memos.perMethod<List<MethodRef>>()
    private val invokeSites = memos.clearedOnEdit<MethodSignature, List<Pair<UInt, Int>>>()
    private val castCounts = memos.clearedOnEdit<CastQuery, Int>()
    private val callers = memos.clearedOnEdit<MethodPattern, Set<UInt>>()
    private val accessors = memos.clearedOnEdit<FieldPattern, Set<UInt>>()
    private val calls = memos.perMethod<MutableMap<MethodPattern, Set<Int>>>()
    private val accesses = memos.perMethod<MutableMap<FieldPattern, Set<Int>>>()

    fun methodRefsOf(method: Method): List<MethodRef> =
        methodRefs.getOrPut(method.handle) { methodReferences(method.handle).distinct() }

    fun calleesOf(method: Method): List<Method> = methodRefsOf(method).mapNotNull(index::methodFor)

    fun calls(method: Method, callee: MethodSignature): Boolean =
        invokeSites[callee]?.any { it.first == method.handle }
            ?: methodRefsOf(method).any { it.signature == callee }

    /** The indices of the instructions in [method] calling a method [pattern] matches. */
    fun callsIn(method: Method, pattern: MethodPattern): Set<Int> =
        calls
            .getOrPut(method.handle) { HashMap() }
            .getOrPut(pattern) {
                findCallsIn(method.handle, pattern).mapTo(linkedSetOf()) { it.toInt() }
            }

    fun methodsCalling(pattern: MethodPattern): Set<UInt> =
        callers.getOrPut(pattern) { findCallsMatching(pattern).mapTo(linkedSetOf()) { it.method } }

    /** The indices of the instructions in [method] accessing a field [pattern] matches. */
    fun accessesIn(method: Method, pattern: FieldPattern): Set<Int> =
        accesses
            .getOrPut(method.handle) { HashMap() }
            .getOrPut(pattern) {
                findFieldAccessesIn(method.handle, pattern).mapTo(linkedSetOf()) { it.toInt() }
            }

    fun methodsAccessing(pattern: FieldPattern): Set<UInt> =
        accessors.getOrPut(pattern) {
            findFieldAccessesMatching(pattern).mapTo(linkedSetOf()) { it.method }
        }

    /** The methods invoking [callee], with the index of each invoke. */
    fun invokeSites(callee: MethodSignature): List<Pair<UInt, Int>> =
        invokeSites.getOrPut(callee) {
            val ref = MethodRef(callee.owner, callee.name, callee.proto)
            findCallsMatching(
                    MethodPattern(
                        ref.definingClass,
                        ref.name,
                        ref.returnType,
                        ref.parameterTypes,
                        emptyList(),
                        null,
                        emptyList(),
                    )
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
