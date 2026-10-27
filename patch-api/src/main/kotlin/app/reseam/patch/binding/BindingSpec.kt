// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.binding

import app.reseam.patch.BindingQuery
import app.reseam.patch.BindingTarget
import app.reseam.patch.ClassTarget
import app.reseam.patch.FieldLocator
import app.reseam.patch.MethodTarget
import app.reseam.patch.PatchRuntime
import app.reseam.patch.PathQuery
import app.reseam.patch.ResolvedBinding
import app.reseam.patch.descriptor
import app.reseam.patch.dex.descriptor
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class BindingSpec : BindingQuery {
    private var fieldLocator: Pair<String?, FieldLocator.() -> Unit>? = null
    private var sourceMethod: MethodTarget? = null
    private var sourceClass: ClassTarget? = null
    private var rawBlock: (PathQuery.() -> Unit)? = null
    private val memberBlocks = linkedMapOf<String, Pair<PathQuery.() -> Unit, BindingTarget?>>()
    private var compiledSourceType: String? = null

    override val sourceType: String
        get() =
            compiledSourceType
                ?: error(
                    "sourceType is only known once the binding's source and raw path are compiled"
                )

    override fun fromField(debugName: String?, block: FieldLocator.() -> Unit) {
        fieldLocator = debugName to block
    }

    override fun fromMethod(target: MethodTarget) {
        sourceMethod = target
    }

    override fun fromClass(target: ClassTarget) {
        sourceClass = target
    }

    override fun raw(block: PathQuery.() -> Unit) {
        rawBlock = block
    }

    override fun member(name: String, block: PathQuery.() -> Unit) {
        memberBlocks[name] = block to null
    }

    override fun bind(name: String, target: BindingTarget, block: PathQuery.() -> Unit) {
        memberBlocks[name] = block to target
    }

    fun compile(runtime: PatchRuntime, label: String): ResolvedBinding {
        val index = runtime.index
        val sourceField = fieldLocator?.let { (debug, block) ->
            FieldLocatorCompiler(index, defaultOwner = null)
                .apply(block)
                .resolveFromAnchor(debug ?: label)
        }
        val anchorMethod = sourceMethod?.let { runtime.resolve(it).value }
        var inputType: String? =
            sourceField?.type ?: sourceClass?.let { runtime.resolve(it).value.descriptor }
        fun defaultRoot(): String =
            inputType
                ?: anchorMethod?.owner
                ?: error("$label has no source. Call fromField, fromMethod, or fromClass first.")

        val members = linkedMapOf<String, ResolvedPath>()
        compiledSourceType = inputType ?: anchorMethod?.owner
        val raw = rawBlock?.let { block ->
            val compiler = PathQueryCompiler(index, anchorMethod, inputType, members)
            compiler.block()
            compiler.build(defaultRoot()).also { inputType = it.rootType }
        }
        val root = defaultRoot()
        val resolvedRaw = raw ?: ResolvedPath(root, root, emptyList())
        compiledSourceType = resolvedRaw.resultType
        for ((name, member) in memberBlocks) {
            val (block, binding) = member
            val compiler =
                PathQueryCompiler(index, anchorMethod, root, members + ("raw" to resolvedRaw))
            compiler.block()
            members[name] = compiler.build(root, binding?.sourceType)
        }
        return ResolvedBinding(resolvedRaw.rootType, resolvedRaw, members.toMap(), sourceField)
    }
}

internal sealed interface PathStep {
    class FieldRead(val field: FieldRef) : PathStep

    class VirtualCall(val ref: MethodRef) : PathStep

    class InterfaceCall(val ref: MethodRef) : PathStep

    class CastValue(val type: String) : PathStep

    class ObjectSlot(val fields: List<FieldRef>, val targetType: String) : PathStep

    data object ListFirst : PathStep

    data object ListLast : PathStep
}

internal class ResolvedPath(val rootType: String, val resultType: String, val steps: List<PathStep>)
