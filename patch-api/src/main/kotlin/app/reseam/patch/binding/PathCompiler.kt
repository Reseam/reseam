// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.binding

import app.reseam.patch.FieldLocator
import app.reseam.patch.FieldTarget
import app.reseam.patch.MethodRank
import app.reseam.patch.MethodRankScope
import app.reseam.patch.PathQuery
import app.reseam.patch.SlotQuery
import app.reseam.patch.Type
import app.reseam.patch.descriptor
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.ref
import app.reseam.patch.dex.returnType
import app.reseam.patch.proto
import app.reseam.patch.search.MethodRankScopeImpl
import app.reseam.patch.search.Ranker
import app.reseam.patch.search.SearchIndex
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class PathQueryCompiler(
    private val index: SearchIndex,
    private val anchorMethod: Method?,
    initialRootType: String?,
    private val knownPaths: Map<String, ResolvedPath>,
) : PathQuery {
    private var rootType: String? = initialRootType
    private var currentType: String? = initialRootType
    private var anchorIndex = -1
    private var anchorCursorType: String? = null
    private val steps = mutableListOf<PathStep>()

    override fun self() {
        require(anchorMethod != null) { "self() requires fromMethod()" }
        require(steps.isEmpty()) { "self() must be the first path step" }
        rootType = anchorMethod.owner
        currentType = rootType
    }

    override fun member(name: String) {
        require(steps.isEmpty()) { "member('$name') must be the first path step" }
        val path = knownPaths[name] ?: error("Unknown path member '$name'")
        rootType = path.rootType
        currentType = path.resultType
        steps += path.steps
    }

    override fun param(index: Int) {
        require(anchorMethod != null) { "param($index) requires fromMethod()" }
        require(steps.isEmpty()) { "param($index) must be the first path step" }
        rootType =
            anchorMethod.parameterTypes.getOrNull(index)
                ?: error("Parameter index $index is out of bounds for ${anchorMethod.descriptor}")
        currentType = rootType
    }

    override fun field(type: String) {
        read(index.requireClass(requireCurrentType()).instanceFieldOfType(descriptor(type)))
    }

    override fun field(name: String, block: FieldLocator.() -> Unit) {
        val owner = requireCurrentType()
        read(FieldLocatorCompiler(index, defaultOwner = owner).apply(block).resolveOnType(name).ref)
    }

    override fun field(target: FieldTarget) = read(target.ref)

    override fun instanceField(typeAnyOf: List<String>) {
        val owner = requireCurrentType()
        val ownerClass = index.requireClass(owner)
        val wanted = typeAnyOf.map(::descriptor)
        val field =
            wanted.firstNotNullOfOrNull { candidate ->
                ownerClass.instanceFields.firstOrNull { it.fieldType == candidate }
            } ?: error("No instance field matching ${wanted.joinToString()} on $owner")
        read(field.ref)
    }

    override fun objectSlots(): SlotQuery =
        object : SlotQuery {
            override fun firstInstanceOf(type: String) {
                val owner = requireCurrentType()
                val ownerClass = index.requireClass(owner)
                val slots =
                    ownerClass.instanceFields
                        .filter { it.fieldType == Type.Object }
                        .map { FieldRef(owner, it.name, it.fieldType) }
                require(slots.isNotEmpty()) { "No Object slot fields found on $owner" }
                val target = descriptor(type)
                add(PathStep.ObjectSlot(slots, target), target)
            }
        }

    override fun firstFieldRead() {
        val method = requireAnchorMethod()
        val insns = method.instructions
        val found =
            insns.objectRead(insns.indices, owner = null)
                ?: error("No object field read found in ${method.descriptor}")
        advance(found)
    }

    override fun nextFieldRead(owner: String?) {
        val method = requireAnchorMethod()
        val expectedOwner = owner?.let(::descriptor) ?: anchorCursorType
        val insns = method.instructions
        val found =
            insns.objectRead(anchorIndex + 1 until insns.size, expectedOwner)
                ?: error(
                    "No object field read found after index $anchorIndex in ${method.descriptor}"
                )
        advance(found)
    }

    private fun advance(found: IndexedValue<FieldRef>) {
        val (index, field) = found
        if (rootType == null && steps.isEmpty()) rootType = field.definingClass
        anchorCursorType = field.fieldType
        anchorIndex = index
        read(field)
    }

    override fun nextInterfaceCall(returning: String?, returningObject: Boolean) {
        val method = requireAnchorMethod()
        val owner =
            anchorCursorType ?: error("nextInterfaceCall() requires a prior field or call step")
        val insns = method.instructions
        val (index, ref) =
            insns.interfaceCall(
                anchorIndex + 1 until insns.size,
                owner,
                returning?.let(::descriptor),
                returningObject,
            )
                ?: error(
                    "No matching interface call found after index $anchorIndex in ${method.descriptor}"
                )
        anchorCursorType = ref.returnType
        anchorIndex = index
        add(PathStep.InterfaceCall(ref), ref.returnType)
    }

    override fun callVirtual(owner: String, name: String, proto: String) {
        val ref = MethodRef(descriptor(owner), name, proto)
        add(PathStep.VirtualCall(ref), ref.returnType)
    }

    override fun callInterface(owner: String, name: String, proto: String) {
        val ref = MethodRef(descriptor(owner), name, proto)
        add(PathStep.InterfaceCall(ref), ref.returnType)
    }

    override fun listGetter(name: String, block: MethodRank.() -> Unit) {
        val owner = requireCurrentType()
        val rankers = mutableListOf<Ranker<MethodRankScope>>()
        object : MethodRank {
                override fun rankBy(label: String, block: MethodRankScope.() -> Int) {
                    rankers += Ranker(label, block)
                }
            }
            .block()
        val ownerClass = index.requireClass(owner)
        val candidates = ownerClass.methods.filter { it.proto == "()${Type.List}" }
        require(candidates.isNotEmpty()) { "No zero-arg List getters found on $owner" }
        val winner = candidates.maxBy { method ->
            rankers.sumOf { it.block(MethodRankScopeImpl(index, method)) }
        }
        val ref = MethodRef(winner.owner, winner.name, winner.proto)
        add(
            if (ownerClass.isInterface) PathStep.InterfaceCall(ref) else PathStep.VirtualCall(ref),
            ref.returnType,
        )
    }

    override fun first() = add(PathStep.ListFirst, Type.Object)

    override fun last() = add(PathStep.ListLast, Type.Object)

    override fun cast(type: String) {
        val target = descriptor(type)
        add(PathStep.CastValue(target), target)
    }

    private fun add(step: PathStep, resultType: String) {
        steps += step
        currentType = resultType
    }

    private fun read(field: FieldRef) = add(PathStep.FieldRead(field), field.fieldType)

    fun build(defaultRootType: String, resultFallback: String? = null) =
        ResolvedPath(
            rootType ?: defaultRootType,
            currentType ?: resultFallback ?: defaultRootType,
            steps.toList(),
        )

    private fun requireCurrentType(): String =
        currentType ?: rootType ?: error("No current type is available for this path")

    private fun requireAnchorMethod(): Method =
        anchorMethod ?: error("This path operation requires fromMethod()")
}
