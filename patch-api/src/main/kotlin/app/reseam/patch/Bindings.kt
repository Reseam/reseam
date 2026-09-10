// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.fieldRef
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType

/**
 * A view over an obfuscated object: a root type found structurally plus named
 * paths (field reads, calls, casts) from it to the values a patch needs.
 * Applied inside code blocks with [of] and [member]. `T` is a marker for the
 * runtime type, used only for readability.
 */
class BindingTarget<T : Any> internal constructor(
    debugName: String?,
    private val block: BindingQuery.() -> Unit,
) : Target<ResolvedBinding>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<ResolvedBinding> {
        val binding = BindingSpec().apply(block).compile(runtime, label)
        return Resolution(binding, wrapped(label, binding.raw.resultType))
    }

    /** The descriptor of the value a path starts from. */
    val sourceType: String get() = resolved.raw.resultType

    /** The field the binding was located through, when it came from `fromField`. */
    val sourceField: FieldTarget
        get() = resolved.sourceField ?: error("$label does not originate from a source field")

    /** `raw` applied to `value`: the bound object. */
    fun of(value: ValueRef): ValueRef = resolved.apply(value, resolved.raw, label)

    fun member(name: String, on: ValueRef): ValueRef {
        val binding = resolved
        if (name == "raw") return of(on)
        val path = binding.members[name] ?: error("$label has no member '$name'; declared: ${binding.members.keys.joinToString()}")
        return binding.apply(on, path, label)
    }
}

fun <T : Any> bind(debugName: String? = null, block: BindingQuery.() -> Unit): BindingTarget<T> =
    BindingTarget(debugName, block)

interface BindingQuery {
    /** The descriptor of the bound object: the source's type inside `raw`, the raw path's result after it. */
    val sourceType: String
    fun fromField(debugName: String? = null, block: FieldLocator.() -> Unit)
    fun fromMethod(target: MethodTarget)
    fun fromClass(target: ClassTarget)
    /** The path from the input value to the bound object. */
    fun raw(block: PathQuery.() -> Unit)
    fun objectValue(name: String, block: PathQuery.() -> Unit)
    fun string(name: String, block: PathQuery.() -> Unit)
    fun context(name: String, block: PathQuery.() -> Unit)
    fun intValue(name: String, block: PathQuery.() -> Unit)
    /** A member whose value is another binding's root when the path does not say otherwise. */
    fun bind(name: String, target: BindingTarget<*>, block: PathQuery.() -> Unit)
}

interface FieldLocator {
    fun owner(type: String)
    fun firstObjectRead()
    fun firstObjectReadAnyOwner()
    fun nearestObjectReadBeforeString(value: String)
    fun rankBy(label: String, block: RankScope.() -> Int)
    fun requireScoreAtLeast(score: Int)
}

interface PathQuery {
    fun self()
    fun member(name: String)
    fun param(index: Int)
    fun field(type: String)
    fun field(name: String, block: FieldLocator.() -> Unit)
    fun field(target: FieldTarget)
    fun instanceField(type: String)
    fun instanceField(typeAnyOf: List<String>)
    fun objectSlots(): SlotQuery
    fun firstFieldRead()
    fun nextFieldRead(owner: String? = null)
    fun nextInterfaceCall(returning: String? = null, returningObject: Boolean = false)
    fun callVirtual(owner: String, name: String, proto: String)
    fun callInterface(owner: String, name: String, proto: String)
    fun listGetter(name: String, block: MethodRank.() -> Unit)
    fun first()
    fun last()
    fun cast(type: String)
}

interface SlotQuery {
    fun firstInstanceOf(type: String)
}

interface MethodRank {
    fun rankBy(label: String, block: MethodRankScope.() -> Int)
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

class ResolvedBinding internal constructor(
    internal val rootType: String,
    internal val raw: ResolvedPath,
    internal val members: Map<String, ResolvedPath>,
    internal val sourceField: FieldTarget?,
) {
    internal fun apply(base: ValueRef, path: ResolvedPath, label: String): ValueRef {
        val emitter = base.emitter()
        val start = with(emitter) { base.impl() }
        var current = adaptInput(emitter, start, path.rootType, label)
        if (path.steps.isEmpty()) return current

        val nullLabel = emitter.nextLabel("binding_null")
        val doneLabel = emitter.nextLabel("binding_done")
        val result = emitter.allocTemp(registerWordCount(path.resultType))

        for (step in path.steps) {
            current = when (step) {
                is PathStep.FieldRead -> emitter.readField(nullChecked(emitter, current, nullLabel), step.field).value(emitter)
                is PathStep.VirtualCall -> emitter.invoke(Opcode.INVOKE_VIRTUAL, step.ref, listOf(nullChecked(emitter, current, nullLabel))).value(emitter)
                is PathStep.InterfaceCall -> emitter.invoke(Opcode.INVOKE_INTERFACE, step.ref, listOf(nullChecked(emitter, current, nullLabel))).value(emitter)
                PathStep.ListFirst -> {
                    emitter.ifZero(current, nullLabel)
                    val empty = emitter.invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "isEmpty", "()Z"), listOf(current)).value(emitter)
                    emitter.ifNonZero(empty, nullLabel)
                    current.get(emitter.int(0)).value(emitter)
                }
                PathStep.ListLast -> {
                    emitter.ifZero(current, nullLabel)
                    current.get(current.size() - emitter.int(1)).value(emitter)
                }
                is PathStep.CastValue -> emitter.cast(current, step.type).value(emitter)
                is PathStep.ObjectSlot -> {
                    val obj = nullChecked(emitter, current, nullLabel)
                    val slotDone = emitter.nextLabel("slot_done")
                    val slot = emitter.allocTemp(constraint = RegisterConstraint.LOW)
                    emitter.constZero(slot, step.targetType)
                    val slotValue = emitter.Value(slot, step.targetType)
                    for ((index, field) in step.fields.withIndex()) {
                        val next = emitter.nextLabel("slot_next_$index")
                        val probe = emitter.readField(obj, field).value(emitter)
                        val isInstance = emitter.instanceOf(probe, step.targetType)
                        emitter.ifZero(isInstance, next)
                        emitter.moveValue(slot, probe.register, step.targetType)
                        emitter.cast(slotValue, step.targetType)
                        emitter.goto(slotDone)
                        emitter.label(next)
                    }
                    emitter.label(slotDone)
                    emitter.ifZero(slotValue, nullLabel)
                    slotValue
                }
            }
        }

        emitter.moveValue(result, current.register, path.resultType)
        emitter.goto(doneLabel)
        emitter.label(nullLabel)
        emitter.constZero(result, path.resultType)
        emitter.label(doneLabel)
        return emitter.Value(result, path.resultType)
    }

    private fun nullChecked(emitter: CodeEmitter, value: CodeEmitter.Value, nullLabel: String): CodeEmitter.Value {
        if (!isReferenceType(value.type)) return value
        val checked = value.asByte()
        emitter.ifZero(checked, nullLabel)
        return checked
    }

    private fun adaptInput(emitter: CodeEmitter, start: CodeEmitter.Value, rootType: String, label: String): CodeEmitter.Value {
        if (!isReferenceType(start.type) || !isReferenceType(rootType) || start.type == rootType) return start
        if (start.type == Type.Object) return emitter.cast(start, rootType).value(emitter)
        val index = ActiveRuntime.current.index
        check(isAssignable(index, start.type, rootType)) {
            "$label expects a value statically assignable to $rootType, got ${start.type}. " +
                "Only Object-typed inputs get the root check-cast implicitly; cast the value first."
        }
        return start
    }

    private fun isAssignable(index: SearchIndex, actual: String, expected: String): Boolean {
        if (actual == expected || expected == Type.Object) return true
        if (actual.startsWith("[") || expected.startsWith("[")) {
            return actual.startsWith("[") && expected in setOf("Ljava/lang/Cloneable;", "Ljava/io/Serializable;")
        }
        val visited = mutableSetOf<String>()
        fun known(descriptor: String): Boolean {
            if (!visited.add(descriptor)) return false
            if (descriptor == expected) return true
            val classDef = index.classFor(descriptor) ?: return false
            if (classDef.interfaces.any { it == expected || known(it) }) return true
            return classDef.superclass?.let(::known) == true
        }
        return known(actual)
    }
}

private fun ValueRef.emitter(): CodeEmitter =
    (this as? CodeEmitter.Value)?.emitter ?: error("Bindings can only be applied inside a code block")

private fun ValueRef.value(emitter: CodeEmitter): CodeEmitter.Value = with(emitter) { impl() }

internal class BindingSpec : BindingQuery {
    private var fieldLocator: Pair<String?, FieldLocator.() -> Unit>? = null
    private var sourceMethod: MethodTarget? = null
    private var sourceClass: ClassTarget? = null
    private var rawBlock: (PathQuery.() -> Unit)? = null
    private val memberBlocks = linkedMapOf<String, Pair<PathQuery.() -> Unit, BindingTarget<*>?>>()
    private var compiledSourceType: String? = null

    override val sourceType: String
        get() = compiledSourceType ?: error("sourceType is only known once the binding's source and raw path are compiled")

    override fun fromField(debugName: String?, block: FieldLocator.() -> Unit) { fieldLocator = debugName to block }
    override fun fromMethod(target: MethodTarget) { sourceMethod = target }
    override fun fromClass(target: ClassTarget) { sourceClass = target }
    override fun raw(block: PathQuery.() -> Unit) { rawBlock = block }
    override fun objectValue(name: String, block: PathQuery.() -> Unit) { memberBlocks[name] = block to null }
    override fun string(name: String, block: PathQuery.() -> Unit) { memberBlocks[name] = block to null }
    override fun context(name: String, block: PathQuery.() -> Unit) { memberBlocks[name] = block to null }
    override fun intValue(name: String, block: PathQuery.() -> Unit) { memberBlocks[name] = block to null }
    override fun bind(name: String, target: BindingTarget<*>, block: PathQuery.() -> Unit) { memberBlocks[name] = block to target }

    fun compile(runtime: PatchRuntime, label: String): ResolvedBinding {
        val index = runtime.index
        val sourceField = fieldLocator?.let { (debug, block) ->
            FieldLocatorCompiler(index, defaultOwner = null).apply(block).resolveFromAnchor(debug ?: label)
        }
        val anchorMethod = sourceMethod?.let { runtime.resolve(it).value }
        var inputType: String? = sourceField?.type ?: sourceClass?.let { runtime.resolve(it).value.descriptor }
        fun defaultRoot(): String = inputType
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
            val compiler = PathQueryCompiler(index, anchorMethod, root, members + ("raw" to resolvedRaw))
            compiler.block()
            members[name] = compiler.build(root, binding?.sourceType)
        }
        return ResolvedBinding(resolvedRaw.rootType, resolvedRaw, members.toMap(), sourceField)
    }
}

private class PathQueryCompiler(
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
        rootType = anchorMethod.parameterTypes.getOrNull(index) ?: error("Parameter index $index is out of bounds for ${anchorMethod.descriptor}")
        currentType = rootType
    }

    override fun field(type: String) {
        val owner = requireCurrentType()
        val field = uniqueField(owner, descriptor(type))
        steps += PathStep.FieldRead(field)
        currentType = field.fieldType
    }

    override fun field(name: String, block: FieldLocator.() -> Unit) {
        val owner = requireCurrentType()
        val field = FieldLocatorCompiler(index, defaultOwner = owner).apply(block).resolveOnType(name).ref
        steps += PathStep.FieldRead(field)
        currentType = field.fieldType
    }

    override fun field(target: FieldTarget) {
        steps += PathStep.FieldRead(target.ref)
        currentType = target.type
    }

    override fun instanceField(type: String) = field(type)

    override fun instanceField(typeAnyOf: List<String>) {
        val owner = requireCurrentType()
        val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
        val wanted = typeAnyOf.map(::descriptor)
        val field = wanted.firstNotNullOfOrNull { candidate -> ownerClass.instanceFields.firstOrNull { it.fieldType == candidate } }
            ?: error("No instance field matching ${wanted.joinToString()} on $owner")
        steps += PathStep.FieldRead(FieldRef(owner, field.name, field.fieldType))
        currentType = field.fieldType
    }

    override fun objectSlots(): SlotQuery = object : SlotQuery {
        override fun firstInstanceOf(type: String) {
            val owner = requireCurrentType()
            val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
            val slots = ownerClass.instanceFields.filter { it.fieldType == Type.Object }.map { FieldRef(owner, it.name, it.fieldType) }
            require(slots.isNotEmpty()) { "No Object slot fields found on $owner" }
            val target = descriptor(type)
            steps += PathStep.ObjectSlot(slots, target)
            currentType = target
        }
    }

    override fun firstFieldRead() {
        val method = requireAnchorMethod()
        val found = findObjectFieldRead(method, 0, owner = null) ?: error("No object field read found in ${method.descriptor}")
        advance(found)
    }

    override fun nextFieldRead(owner: String?) {
        val method = requireAnchorMethod()
        val expectedOwner = owner?.let(::descriptor) ?: anchorCursorType
        val found = findObjectFieldRead(method, anchorIndex + 1, expectedOwner)
            ?: error("No object field read found after index $anchorIndex in ${method.descriptor}")
        advance(found)
    }

    private fun advance(found: Pair<Int, FieldRef>) {
        if (rootType == null && steps.isEmpty()) rootType = found.second.definingClass
        currentType = found.second.fieldType
        anchorCursorType = found.second.fieldType
        anchorIndex = found.first
        steps += PathStep.FieldRead(found.second)
    }

    override fun nextInterfaceCall(returning: String?, returningObject: Boolean) {
        val method = requireAnchorMethod()
        val owner = anchorCursorType ?: error("nextInterfaceCall() requires a prior field or call step")
        val insns = method.instructions
        val wanted = returning?.let(::descriptor)
        val found = (anchorIndex + 1 until insns.size).firstNotNullOfOrNull { i ->
            val insn = insns[i]
            val ref = insn.methodRef ?: return@firstNotNullOfOrNull null
            val interfaceCall = insn.opcode == Opcode.INVOKE_INTERFACE || insn.opcode == Opcode.INVOKE_INTERFACE_RANGE
            if (!interfaceCall || ref.definingClass != owner) return@firstNotNullOfOrNull null
            if (wanted != null && ref.proto != wanted) return@firstNotNullOfOrNull null
            if (returningObject && !isReferenceType(ref.returnType)) return@firstNotNullOfOrNull null
            i to ref
        } ?: error("No matching interface call found after index $anchorIndex in ${method.descriptor}")
        currentType = found.second.returnType
        anchorCursorType = currentType
        anchorIndex = found.first
        steps += PathStep.InterfaceCall(found.second)
    }

    override fun callVirtual(owner: String, name: String, proto: String) {
        val ref = MethodRef(descriptor(owner), name, proto)
        steps += PathStep.VirtualCall(ref)
        currentType = ref.returnType
    }

    override fun callInterface(owner: String, name: String, proto: String) {
        val ref = MethodRef(descriptor(owner), name, proto)
        steps += PathStep.InterfaceCall(ref)
        currentType = ref.returnType
    }

    override fun listGetter(name: String, block: MethodRank.() -> Unit) {
        val owner = requireCurrentType()
        val rankers = mutableListOf<Ranker<MethodRankScope>>()
        object : MethodRank {
            override fun rankBy(label: String, block: MethodRankScope.() -> Int) { rankers += Ranker(label, block) }
        }.block()
        val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
        val candidates = ownerClass.methods.filter { it.proto == "()${Type.List}" }
        require(candidates.isNotEmpty()) { "No zero-arg List getters found on $owner" }
        val winner = candidates.maxBy { method -> rankers.sumOf { it.block(MethodRankScopeImpl(index, method)) } }
        val ref = MethodRef(winner.owner, winner.name, winner.proto)
        steps += if (ownerClass.isInterface) PathStep.InterfaceCall(ref) else PathStep.VirtualCall(ref)
        currentType = ref.returnType
    }

    override fun first() {
        steps += PathStep.ListFirst
        currentType = Type.Object
    }

    override fun last() {
        steps += PathStep.ListLast
        currentType = Type.Object
    }

    override fun cast(type: String) {
        val target = descriptor(type)
        steps += PathStep.CastValue(target)
        currentType = target
    }

    fun build(defaultRootType: String, resultFallback: String? = null) =
        ResolvedPath(rootType ?: defaultRootType, currentType ?: resultFallback ?: defaultRootType, steps.toList())

    private fun requireCurrentType(): String = currentType ?: rootType ?: error("No current type is available for this path")

    private fun requireAnchorMethod(): Method = anchorMethod ?: error("This path operation requires fromMethod()")

    private fun uniqueField(owner: String, type: String): FieldRef {
        val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
        val matches = ownerClass.instanceFields.filter { it.fieldType == type }
        require(matches.size == 1) { "Expected exactly one field of type $type on $owner, found ${matches.size}" }
        return FieldRef(owner, matches.single().name, type)
    }

    private fun findObjectFieldRead(method: Method, start: Int, owner: String?): Pair<Int, FieldRef>? {
        val insns = method.instructions
        for (i in start until insns.size) {
            val insn = insns[i]
            if (insn.opcode != Opcode.IGET_OBJECT) continue
            val field = insn.fieldRef ?: continue
            if (owner != null && field.definingClass != owner) continue
            return i to field
        }
        return null
    }
}

private class FieldLocatorCompiler(
    private val index: SearchIndex,
    private val defaultOwner: String?,
) : FieldLocator {
    private var ownerDescriptor: String? = null
    private var firstObjectReadMode = 0
    private var nearestBeforeString: String? = null
    private val rankers = mutableListOf<Ranker<RankScope>>()
    private var minScore = Int.MIN_VALUE

    override fun owner(type: String) { ownerDescriptor = descriptor(type) }
    override fun firstObjectRead() { firstObjectReadMode = 1 }
    override fun firstObjectReadAnyOwner() { firstObjectReadMode = 2 }
    override fun nearestObjectReadBeforeString(value: String) { nearestBeforeString = value }
    override fun rankBy(label: String, block: RankScope.() -> Int) { rankers += Ranker(label, block) }
    override fun requireScoreAtLeast(score: Int) { minScore = score }

    fun resolveFromAnchor(label: String): FieldTarget {
        val owner = ownerDescriptor ?: defaultOwner ?: error("Field locator '$label' requires owner(...)")
        val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
        nearestBeforeString?.let { needle ->
            var best: Pair<FieldRef, Int>? = null
            for (method in ownerClass.methods) {
                val anchor = method.indexOfFirstString(needle) ?: continue
                val insns = method.instructions
                for (i in (anchor - 1) downTo 0) {
                    val insn = insns[i]
                    if (insn.opcode != Opcode.IGET_OBJECT) continue
                    val field = insn.fieldRef ?: continue
                    if (field.definingClass != owner) continue
                    val distance = anchor - i
                    if (best == null || distance < best.second) best = field to distance
                    break
                }
            }
            return FieldTarget.of(best?.first ?: error("No object field read before '$needle' on $owner"), label)
        }
        if (firstObjectReadMode != 0) {
            val fieldOwner = if (firstObjectReadMode == 1) owner else null
            for (method in ownerClass.methods) {
                for (insn in method.instructions) {
                    if (insn.opcode != Opcode.IGET_OBJECT) continue
                    val field = insn.fieldRef ?: continue
                    if (fieldOwner != null && field.definingClass != fieldOwner) continue
                    return FieldTarget.of(field, label)
                }
            }
            error("No object field read found on $owner")
        }
        return resolveOnType(label)
    }

    fun resolveOnType(label: String): FieldTarget {
        val owner = ownerDescriptor ?: defaultOwner ?: error("No owner type is available for field '$label'")
        val ownerClass = index.classFor(owner) ?: error("Class not found: $owner")
        val candidates = ownerClass.instanceFields.map { field ->
            FieldRef(owner, field.name, field.fieldType) to rankers.sumOf { it.block(RankScopeImpl(index, field.fieldType)) }
        }
        require(candidates.isNotEmpty()) { "No instance fields found on $owner" }
        val winner = candidates.maxBy { it.second }
        require(winner.second >= minScore) { "Best field candidate for '$label' on $owner scored ${winner.second}, below required $minScore" }
        if (rankers.isEmpty()) require(candidates.size == 1) { "Field '$label' on $owner is ambiguous without ranking; found ${candidates.size} candidates" }
        return FieldTarget.of(winner.first, label)
    }
}
