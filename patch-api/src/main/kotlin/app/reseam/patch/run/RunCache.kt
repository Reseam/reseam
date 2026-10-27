// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.run

import app.reseam.patch.native.getClassInfo
import app.reseam.patch.native.getClassInfos
import app.reseam.patch.native.getInstructions
import app.reseam.patch.native.getMethodInfo
import app.reseam.patch.native.getMethodInfos
import app.reseam.patch.native.mutationChanges
import app.reseam.patch.native.mutationRevision
import app.reseam.patch.search.SearchIndex
import app.reseam.patch.types.ClassInfo
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo

/**
 * Bytecode metadata and search results shared by every patch invocation in a run. Engine handles
 * stay valid for the whole run, and the engine's mutation journal says what each edit made stale.
 */
internal class RunCache {
    private var revision: ULong? = null
    val memos = Memos(::synchronize)
    private val methodInfos = memos.perMethod<MethodInfo>()
    private val instructions = memos.perMethod<List<Instruction>>()
    private val classInfos = memos.keptOnEdit<UInt, ClassInfo>()

    val index: SearchIndex by lazy { SearchIndex(this) }

    fun synchronize() {
        val current = mutationRevision()
        val seen = revision
        revision = current
        if (seen == null || current == seen) return
        mutationChanges(seen)?.let(memos::edited) ?: memos.clear()
    }

    fun methodInfo(handle: UInt): MethodInfo =
        methodInfos.getOrPut(handle) {
            getMethodInfo(handle) ?: error("invalid method handle: $handle")
        }

    fun instructions(handle: UInt): List<Instruction> =
        instructions.getOrPut(handle) { getInstructions(handle) }

    fun classInfo(handle: UInt): ClassInfo =
        classInfos.getOrPut(handle) {
            getClassInfo(handle) ?: error("invalid class handle: $handle")
        }

    fun prefetchMethods(handles: Collection<UInt>) =
        prefetch(handles, methodInfos, ::getMethodInfos)

    fun prefetchClasses(handles: Collection<UInt>) = prefetch(handles, classInfos, ::getClassInfos)

    private fun <V> prefetch(
        handles: Collection<UInt>,
        memo: Memos.Memo<UInt, V>,
        fetch: (UIntArray) -> List<V>,
    ) =
        memo.missing(handles).chunked(256).forEach { batch ->
            val infos = fetch(batch.toUIntArray())
            check(infos.size == batch.size) { "Invalid handle in query candidates" }
            batch.zip(infos).forEach { (handle, info) -> memo[handle] = info }
        }
}
