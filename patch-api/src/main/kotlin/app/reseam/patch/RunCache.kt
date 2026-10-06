// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.getClassInfo
import app.reseam.patch.native.getClassInfos
import app.reseam.patch.native.getInstructions
import app.reseam.patch.native.getMethodInfo
import app.reseam.patch.native.getMethodInfos
import app.reseam.patch.native.mutationChanges
import app.reseam.patch.native.mutationRevision
import app.reseam.patch.types.ClassInfo
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.MethodInfo

/**
 * Bytecode metadata and search results shared by every patch invocation in a run. Engine handles
 * stay valid for the whole run, and the engine's mutation journal says what each edit made stale.
 */
internal class RunCache {
    private val methodInfos = HashMap<UInt, MethodInfo>()
    private val classInfos = HashMap<UInt, ClassInfo>()
    private val instructions = HashMap<UInt, List<Instruction>>()
    private var revision: ULong? = null
    private var searchIndex: SearchIndex? = null

    val index: SearchIndex
        get() {
            synchronize()
            return searchIndex ?: SearchIndex(this).also { searchIndex = it }
        }

    fun synchronize() {
        val current = mutationRevision()
        val seen = revision
        revision = current
        if (seen == null || current == seen) return
        val changes = mutationChanges(seen)
        if (changes == null) {
            methodInfos.clear()
            classInfos.clear()
            instructions.clear()
            searchIndex?.invalidate()
        } else {
            changes.forEach {
                methodInfos.remove(it)
                instructions.remove(it)
            }
            searchIndex?.invalidateMethods(changes)
        }
    }

    fun methodInfo(handle: UInt): MethodInfo {
        synchronize()
        return methodInfos.getOrPut(handle) {
            getMethodInfo(handle) ?: error("invalid method handle: $handle")
        }
    }

    fun instructions(handle: UInt): List<Instruction> {
        synchronize()
        return instructions.getOrPut(handle) { getInstructions(handle) }
    }

    fun classInfo(handle: UInt): ClassInfo {
        synchronize()
        return classInfos.getOrPut(handle) {
            getClassInfo(handle) ?: error("invalid class handle: $handle")
        }
    }

    fun prefetchMethods(handles: Collection<UInt>) {
        synchronize()
        handles.filterNot(methodInfos::containsKey).chunked(256).forEach { batch ->
            val infos = getMethodInfos(batch.toUIntArray())
            check(infos.size == batch.size) { "Invalid method in query candidates" }
            batch.zip(infos).forEach { (handle, info) -> methodInfos[handle] = info }
        }
    }

    fun prefetchClasses(handles: Collection<UInt>) {
        synchronize()
        handles.filterNot(classInfos::containsKey).chunked(256).forEach { batch ->
            val infos = getClassInfos(batch.toUIntArray())
            check(infos.size == batch.size) { "Invalid class in query candidates" }
            batch.zip(infos).forEach { (handle, info) -> classInfos[handle] = info }
        }
    }
}
