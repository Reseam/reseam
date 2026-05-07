// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import kotlin.reflect.KClass

private const val ACCESS_INTERFACE = 0x0200
private const val OBJECT_TYPE = "Ljava/lang/Object;"
private const val LIST_TYPE = "Ljava/util/List;"
private const val CLONEABLE_TYPE = "Ljava/lang/Cloneable;"
private const val SERIALIZABLE_TYPE = "Ljava/io/Serializable;"

internal class BindingCompiler<T : Any>(
    private val state: PatchApiState,
    private val runtimeTypeRef: KClass<T>,
    private val debug: String?,
) : BindingQuery<T> {
    private var sourceFieldHandle: FieldHandle? = null
    private var sourceMethodHandle: MethodHandle? = null
    private var sourceClassHandle: ClassHandle? = null
    private var inputType: String? = null
    private var rawPath: ResolvedPath? = null
    private val members = linkedMapOf<String, ResolvedPath>()

    override val sourceType: String
        get() = rawPath?.resultType ?: defaultInputType()

    override fun fromField(debug: String?, block: FieldLocator.() -> Unit) {
        val resolver = FieldLocatorCompiler(state, defaultOwner = null).apply(block)
        val field = resolver.resolveFromAnchor(debug)
        sourceFieldHandle = field
        inputType = field.type
    }

    override fun fromMethod(debug: String?, block: MethodQuery.() -> Unit) {
        sourceMethodHandle = state.findMethod(debug, block)
    }

    override fun fromClass(target: ClassHandle) {
        sourceClassHandle = target
        inputType = target.descriptor
    }

    override fun raw(block: PathQuery.() -> Unit) {
        val anchorMethod = sourceMethodHandle?.method
        val compiler = PathQueryCompiler(
            state = state,
            anchorMethod = anchorMethod,
            initialRootType = inputType,
            knownPaths = members + listOfNotNull(rawPath?.let { "raw" to it }).toMap(),
        )
        compiler.block()
        val resolved = compiler.build(defaultRootType())
        rawPath = resolved
        inputType = resolved.rootType
    }

    override fun objectValue(name: String, block: PathQuery.() -> Unit) {
        members[name] = compileMemberPath(block)
    }

    override fun string(name: String, block: PathQuery.() -> Unit) {
        members[name] = compileMemberPath(block)
    }

    override fun context(name: String, block: PathQuery.() -> Unit) {
        members[name] = compileMemberPath(block)
    }

    override fun bind(name: String, target: Binding<*>, block: PathQuery.() -> Unit) {
        members[name] = compileMemberPath(block)
    }

    override fun list(name: String, element: Binding<*>, block: PathQuery.() -> Unit) {
        members[name] = compileMemberPath(block)
    }

    fun build(): Binding<T> {
        val root = defaultRootType()
        val resolvedRaw = rawPath ?: ResolvedPath(rootType = root, resultType = root, steps = emptyList())
        return BindingImpl(
            state = state,
            debugName = debug,
            runtimeType = runtimeTypeRef,
            rootType = resolvedRaw.rootType,
            raw = resolvedRaw,
            members = members.toMap(),
            sourceFieldHandle = sourceFieldHandle,
        )
    }

    private fun compileMemberPath(block: PathQuery.() -> Unit): ResolvedPath {
        val compiler = PathQueryCompiler(
            state = state,
            anchorMethod = sourceMethodHandle?.method,
            initialRootType = defaultRootType(),
            knownPaths = members + listOfNotNull(rawPath?.let { "raw" to it }).toMap(),
        )
        compiler.block()
        return compiler.build(defaultRootType())
    }

    private fun defaultRootType(): String =
        inputType
            ?: sourceMethodHandle?.classDescriptor
            ?: sourceClassHandle?.descriptor
            ?: sourceFieldHandle?.type
            ?: error("Binding '$debug' has no source. Call fromField/fromMethod/fromClass first.")

    private fun defaultInputType(): String =
        rawPath?.rootType ?: defaultRootType()
}

private class BindingImpl<T : Any>(
    private val state: PatchApiState,
    override val debugName: String?,
    override val runtimeType: KClass<T>,
    private val rootType: String,
    private val raw: ResolvedPath,
    private val members: Map<String, ResolvedPath>,
    private val sourceFieldHandle: FieldHandle?,
) : Binding<T> {
    override val sourceType: String
        get() = raw.resultType

    override fun of(raw: ValueRef): ValueRef =
        applyPath(raw, this.raw)

    override fun member(name: String, on: ValueRef): ValueRef {
        if (name == "raw") return of(on)
        val path = members[name] ?: error("Binding '${debugName ?: runtimeType.simpleName}' has no member '$name'")
        return applyPath(on, path)
    }

    override fun sourceField(): FieldHandle =
        sourceFieldHandle ?: error("Binding '${debugName ?: runtimeType.simpleName}' does not originate from a source field")

    private fun applyPath(base: ValueRef, path: ResolvedPath): ValueRef {
        val start = base as? ValueRefImpl ?: error("Bindings can only be used from inside CodeScope")
        var current = adaptInputToRootType(start, path.rootType)
        if (path.steps.isEmpty()) return current

        val emitter = current.emitter
        val nullLabel = emitter.nextLabel("binding_null")
        val doneLabel = emitter.nextLabel("binding_done")
        val result = emitter.allocTemp(registerWordCount(path.resultType))

        for (step in path.steps) {
            current = when (step) {
                is RuntimePathStep.FieldRead -> {
                    current = nullChecked(current, nullLabel)
                    emitter.valueField(current, step.field.field).asImpl()
                }
                is RuntimePathStep.VirtualCall -> {
                    current = nullChecked(current, nullLabel)
                    emitter.invoke(
                        step.owner,
                        step.name,
                        step.proto,
                        listOf(current),
                        InvokeKind.VIRTUAL,
                    ).asImpl()
                }
                is RuntimePathStep.InterfaceCall -> {
                    current = nullChecked(current, nullLabel)
                    emitter.invoke(
                        step.owner,
                        step.name,
                        step.proto,
                        listOf(current),
                        InvokeKind.INTERFACE,
                    ).asImpl()
                }
                RuntimePathStep.ListFirst -> {
                    current = emitter.byteRegister(current)
                    emitter.builder.ifEqz(current.register, nullLabel)
                    val empty = emitter.invoke(LIST_TYPE, "isEmpty", "()Z", listOf(current), InvokeKind.INTERFACE).asImpl()
                    emitter.builder.ifNez(emitter.byteRegister(empty).register, nullLabel)
                    emitter.listGet(current, emitter.int(0).asImpl()).asImpl()
                }
                RuntimePathStep.ListLast -> {
                    current = emitter.byteRegister(current)
                    emitter.builder.ifEqz(current.register, nullLabel)
                    val size = emitter.listSize(current).asImpl()
                    val lastIndex = emitter.subtract(size, emitter.int(1).asImpl()).asImpl()
                    emitter.listGet(current, lastIndex).asImpl()
                }
                is RuntimePathStep.CastValue -> emitter.cast(current, step.type).asImpl()
                is RuntimePathStep.ObjectSlot -> {
                    current = emitter.lowRegister(nullChecked(current, nullLabel))
                    val slotDone = emitter.nextLabel("slot_done")
                    val slotValue = emitter.allocTemp(constraint = RegisterConstraint.LOW)
                    emitter.builder.const4(slotValue, 0)
                    for ((index, field) in step.fields.withIndex()) {
                        val probe = emitter.allocTemp(constraint = RegisterConstraint.LOW)
                        val probeType = emitter.allocTemp(constraint = RegisterConstraint.LOW)
                        val nextLabel = emitter.nextLabel("slot_next_$index")
                        emitter.builder.igetObject(probe, current.register, field.field)
                        emitter.builder.instanceOf(probeType, probe, step.targetType)
                        emitter.builder.ifEqz(probeType, nextLabel)
                        emitter.builder.moveObject(slotValue, probe)
                        emitter.builder.checkCast(slotValue, step.targetType)
                        emitter.builder.goto(slotDone)
                        emitter.builder.label(nextLabel)
                    }
                    emitter.builder.label(slotDone)
                    emitter.builder.ifEqz(slotValue, nullLabel)
                    ValueRefImpl(emitter, slotValue, step.targetType)
                }
            }
        }

        emitter.moveValue(result, current.register, path.resultType)
        emitter.builder.goto(doneLabel)
        emitter.builder.label(nullLabel)
        if (registerWordCount(path.resultType) == 2) {
            emitter.builder.constWide16(result, 0)
        } else {
            emitter.builder.const16(result, 0)
        }
        emitter.builder.label(doneLabel)
        return ValueRefImpl(emitter, result, path.resultType)
    }

    private fun nullChecked(value: ValueRefImpl, nullLabel: String): ValueRefImpl {
        if (!isReferenceType(value.type)) return value
        val checked = value.emitter.byteRegister(value)
        value.emitter.builder.ifEqz(checked.register, nullLabel)
        return checked
    }

    private fun adaptInputToRootType(start: ValueRefImpl, rootType: String): ValueRefImpl {
        if (!isReferenceType(start.type) || !isReferenceType(rootType) || start.type == rootType) {
            return start
        }
        if (start.type == OBJECT_TYPE) {
            return start.emitter.cast(start, rootType).asImpl()
        }
        return when (rootCompatibility(start.type, rootType)) {
            BindingRootCompatibility.Assignable -> start
            BindingRootCompatibility.Unknown,
            BindingRootCompatibility.Incompatible,
            ->
                error(
                    "Binding '${bindingName()}' expects a value statically assignable to $rootType, got ${start.type}. " +
                        "The engine only inserts the root check-cast implicitly for Object-typed inputs. " +
                        "Pass an Object-typed stub parameter or cast the value explicitly before applying the binding."
                )
        }
    }

    private fun rootCompatibility(actualType: String, expectedType: String): BindingRootCompatibility {
        if (actualType == expectedType) return BindingRootCompatibility.Assignable
        if (expectedType == OBJECT_TYPE && isReferenceType(actualType)) return BindingRootCompatibility.Assignable

        if (actualType.startsWith("[") || expectedType.startsWith("[")) {
            return when {
                actualType == expectedType -> BindingRootCompatibility.Assignable
                expectedType == OBJECT_TYPE && isReferenceType(actualType) -> BindingRootCompatibility.Assignable
                actualType.startsWith("[") && expectedType in setOf(CLONEABLE_TYPE, SERIALIZABLE_TYPE) ->
                    BindingRootCompatibility.Assignable
                else -> BindingRootCompatibility.Unknown
            }
        }

        val actualClass = state.classFor(actualType) ?: return BindingRootCompatibility.Unknown
        if (isKnownAssignable(actualClass, expectedType, mutableSetOf())) {
            return BindingRootCompatibility.Assignable
        }

        return if (state.classFor(expectedType) != null) {
            BindingRootCompatibility.Incompatible
        } else {
            BindingRootCompatibility.Unknown
        }
    }

    private fun isKnownAssignable(classDef: DexClass, expectedType: String, visited: MutableSet<String>): Boolean {
        val descriptor = classDef.info.descriptor
        if (!visited.add(descriptor)) return false
        if (descriptor == expectedType) return true

        if (classDef.info.interfaces.any { it == expectedType }) return true
        for (iface in classDef.info.interfaces) {
            val ifaceClass = state.classFor(iface) ?: continue
            if (isKnownAssignable(ifaceClass, expectedType, visited)) return true
        }

        val superclass = classDef.superclass ?: return false
        if (superclass == expectedType) return true
        val superClassDef = state.classFor(superclass) ?: return false
        return isKnownAssignable(superClassDef, expectedType, visited)
    }

    private fun bindingName(): String =
        debugName ?: runtimeType.simpleName ?: rootType
}

private enum class BindingRootCompatibility {
    Assignable,
    Unknown,
    Incompatible,
}

private data class ResolvedPath(
    val rootType: String,
    val resultType: String,
    val steps: List<RuntimePathStep>,
)

private sealed interface RuntimePathStep {
    data class FieldRead(val field: FieldHandle) : RuntimePathStep
    data class VirtualCall(val owner: String, val name: String, val proto: String) : RuntimePathStep
    data class InterfaceCall(val owner: String, val name: String, val proto: String) : RuntimePathStep
    data class CastValue(val type: String) : RuntimePathStep
    data class ObjectSlot(val fields: List<FieldHandle>, val targetType: String) : RuntimePathStep

    data object ListFirst : RuntimePathStep
    data object ListLast : RuntimePathStep
}

private class PathQueryCompiler(
    private val state: PatchApiState,
    private val anchorMethod: Method?,
    initialRootType: String?,
    private val knownPaths: Map<String, ResolvedPath>,
) : PathQuery {
    private var rootType: String? = initialRootType
    private var currentType: String? = initialRootType
    private var anchorIndex: Int = -1
    private var anchorCursorType: String? = null
    private val steps = mutableListOf<RuntimePathStep>()
    private val slotQuery = SlotQueryImpl(this)

    override fun self() {
        require(anchorMethod != null) { "self() requires fromMethod()" }
        require(steps.isEmpty()) { "self() must be the first path step" }
        rootType = anchorMethod.info.classDescriptor
        currentType = rootType
    }

    override fun member(name: String) {
        require(steps.isEmpty()) {
            "member('$name') must be the first path step in the current path"
        }
        val path = knownPaths[name] ?: error("Unknown path member '$name'")
        rootType = path.rootType
        currentType = path.resultType
        steps += path.steps
    }

    override fun parameter(index: Int) {
        require(anchorMethod != null) { "parameter($index) requires fromMethod()" }
        require(steps.isEmpty()) { "parameter($index) must be the first path step" }
        rootType = anchorMethod.parameterTypes.getOrNull(index)
            ?: error("Parameter index $index is out of bounds for ${anchorMethod.info.classDescriptor}->${anchorMethod.info.methodName}${anchorMethod.info.proto}")
        currentType = rootType
    }

    override fun field(type: String) {
        val owner = requireCurrentType()
        val field = resolveUniqueField(owner, type)
        steps += RuntimePathStep.FieldRead(field)
        currentType = field.type
    }

    override fun field(name: String, block: FieldLocator.() -> Unit) {
        val owner = requireCurrentType()
        val resolver = FieldLocatorCompiler(state, defaultOwner = owner).apply(block)
        val field = resolver.resolveOnType(name)
        steps += RuntimePathStep.FieldRead(field)
        currentType = field.type
    }

    override fun field(ref: FieldHandle) {
        steps += RuntimePathStep.FieldRead(ref)
        currentType = ref.type
    }

    override fun instanceField(type: String) {
        field(type)
    }

    override fun instanceField(typeAnyOf: List<String>) {
        val owner = requireCurrentType()
        val ownerClass = state.classFor(owner) ?: error("Class not found: $owner")
        val field = typeAnyOf.firstNotNullOfOrNull { candidate ->
            ownerClass.instanceFields.firstOrNull { it.fieldType == candidate }
                ?.let { state.wrapField(FieldRef(owner, it.name, it.fieldType), debug = "$owner:${it.name}") }
        } ?: error("No instance field matching ${typeAnyOf.joinToString()} on $owner")
        steps += RuntimePathStep.FieldRead(field)
        currentType = field.type
    }

    override fun objectSlots(): SlotQuery = slotQuery

    override fun firstFieldRead() {
        val method = requireAnchorMethod()
        val found = findFirstObjectFieldRead(method, 0, owner = null)
            ?: error("No object field read found in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}")
        if (rootType == null && steps.isEmpty()) {
            rootType = found.owner
        }
        currentType = found.type
        anchorCursorType = found.type
        anchorIndex = found.index
        steps += RuntimePathStep.FieldRead(found.handle)
    }

    override fun nextFieldRead(owner: String?) {
        val method = requireAnchorMethod()
        val expectedOwner = owner ?: anchorCursorType
        val found = findFirstObjectFieldRead(method, anchorIndex + 1, expectedOwner)
            ?: error("No object field read found after index $anchorIndex in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}")
        if (rootType == null && steps.isEmpty()) {
            rootType = found.owner
        }
        currentType = found.type
        anchorCursorType = found.type
        anchorIndex = found.index
        steps += RuntimePathStep.FieldRead(found.handle)
    }

    override fun nextInterfaceCall(returning: String?, returningObject: Boolean) {
        val method = requireAnchorMethod()
        val owner = anchorCursorType ?: error("nextInterfaceCall() requires a prior field/call step")
        val found = findNextInterfaceCall(method, anchorIndex + 1, owner, returning, returningObject)
            ?: error("No matching interface call found after index $anchorIndex in ${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}")
        currentType = found.ref.returnType
        anchorCursorType = currentType
        anchorIndex = found.index
        steps += RuntimePathStep.InterfaceCall(found.ref.definingClass, found.ref.name, found.ref.proto)
    }

    override fun callVirtual(owner: String, name: String, proto: String) {
        steps += RuntimePathStep.VirtualCall(owner, name, proto)
        currentType = proto.substringAfter(')')
    }

    override fun callInterface(owner: String, name: String, proto: String) {
        steps += RuntimePathStep.InterfaceCall(owner, name, proto)
        currentType = proto.substringAfter(')')
    }

    override fun listGetter(name: String, block: MethodRank.() -> Unit) {
        val owner = requireCurrentType()
        val method = MethodRankCompiler(state, owner).apply(block).resolve()
        if (isInterface(owner)) {
            steps += RuntimePathStep.InterfaceCall(method.definingClass, method.name, method.proto)
        } else {
            steps += RuntimePathStep.VirtualCall(method.definingClass, method.name, method.proto)
        }
        currentType = method.returnType
    }

    override fun first() {
        steps += RuntimePathStep.ListFirst
        currentType = OBJECT_TYPE
    }

    override fun last() {
        steps += RuntimePathStep.ListLast
        currentType = OBJECT_TYPE
    }

    override fun cast(type: String) {
        steps += RuntimePathStep.CastValue(type)
        currentType = type
    }

    override fun asBinding(binding: Binding<*>) {
        if (currentType == null) {
            currentType = binding.sourceType
        }
    }

    fun build(defaultRootType: String): ResolvedPath =
        ResolvedPath(
            rootType = rootType ?: defaultRootType,
            resultType = currentType ?: defaultRootType,
            steps = steps.toList(),
        )

    internal fun objectSlotsFirstInstanceOf(type: String) {
        val owner = requireCurrentType()
        val ownerClass = state.classFor(owner) ?: error("Class not found: $owner")
        val slots = ownerClass.instanceFields
            .filter { it.fieldType == OBJECT_TYPE }
            .map { state.wrapField(FieldRef(owner, it.name, it.fieldType), debug = "$owner:${it.name}") }
        require(slots.isNotEmpty()) { "No Object slot fields found on $owner" }
        steps += RuntimePathStep.ObjectSlot(slots, type)
        currentType = type
    }

    private fun requireCurrentType(): String =
        currentType ?: rootType ?: error("No current type is available for this path")

    private fun requireAnchorMethod(): Method =
        anchorMethod ?: error("This path operation requires fromMethod()")

    private fun resolveUniqueField(owner: String, type: String): FieldHandle {
        val ownerClass = state.classFor(owner) ?: error("Class not found: $owner")
        val matches = ownerClass.instanceFields.filter { it.fieldType == type }
        require(matches.size == 1) {
            "Expected exactly one field of type $type on $owner, found ${matches.size}"
        }
        return state.wrapField(FieldRef(owner, matches.single().name, matches.single().fieldType), debug = "$owner:${matches.single().name}")
    }

    private fun findFirstObjectFieldRead(method: Method, start: Int, owner: String?): FieldReadResult? {
        for (index in start until method.instructions.size) {
            val instruction = method.instructions[index]
            if (instruction !is Instruction.RegField || instruction.opcode() != Opcodes.IGET_OBJECT) continue
            val field = instruction.value0.field
            if (owner != null && field.definingClass != owner) continue
            return FieldReadResult(
                index = index,
                owner = field.definingClass,
                type = field.fieldType,
                handle = state.wrapField(field, debug = "${field.definingClass}:${field.name}"),
            )
        }
        return null
    }

    private fun findNextInterfaceCall(
        method: Method,
        start: Int,
        owner: String,
        returning: String?,
        returningObject: Boolean,
    ): MethodReadResult? {
        for (index in start until method.instructions.size) {
            val ref = method.instructions[index].methodRef() ?: continue
            val opcode = method.instructions[index].opcode()
            if (opcode != Opcodes.INVOKE_INTERFACE && opcode != Opcodes.INVOKE_INTERFACE_RANGE) continue
            if (ref.definingClass != owner) continue
            if (returning != null && ref.proto != returning) continue
            if (returningObject && !(ref.returnType.startsWith("L") || ref.returnType.startsWith("["))) continue
            return MethodReadResult(index, ref)
        }
        return null
    }

    private fun isInterface(descriptor: String): Boolean =
        ((state.classFor(descriptor)?.info?.accessFlags?.toInt() ?: 0) and ACCESS_INTERFACE) != 0
}

private class SlotQueryImpl(
    private val compiler: PathQueryCompiler,
) : SlotQuery {
    override fun firstInstanceOf(type: String) {
        compiler.objectSlotsFirstInstanceOf(type)
    }
}

private class MethodRankCompiler(
    private val state: PatchApiState,
    private val ownerType: String,
) : MethodRank {
    private val rankers = mutableListOf<Pair<String, RankScope.() -> Int>>()

    override fun rankBy(label: String, block: RankScope.() -> Int) {
        rankers += label to block
    }

    fun resolve(): MethodRef {
        val owner = state.classFor(ownerType) ?: error("Class not found: $ownerType")
        val candidates = owner.methods.filter { it.info.proto == "()$LIST_TYPE" }
        require(candidates.isNotEmpty()) { "No zero-arg List getters found on $ownerType" }
        val winner = candidates.maxByOrNull { method ->
            rankers.sumOf { (_, ranker) -> ranker(RankScopeImpl(state, method = method)) }
        } ?: error("No List getter candidates for $ownerType")
        return MethodRef(winner.info.classDescriptor, winner.info.methodName, winner.info.proto)
    }
}

private class FieldLocatorCompiler(
    private val state: PatchApiState,
    private val defaultOwner: String?,
) : FieldLocator {
    private var ownerDescriptor: String? = null
    private var firstObjectReadMode = FirstObjectReadMode.None
    private var nearestBeforeString: String? = null
    private val rankers = mutableListOf<Pair<String, RankScope.() -> Int>>()
    private var minScore = Int.MIN_VALUE

    override fun owner(classDescriptor: String) {
        ownerDescriptor = classDescriptor
    }

    override fun firstObjectRead() {
        firstObjectReadMode = FirstObjectReadMode.DeclaredOnOwner
    }

    override fun firstObjectReadAnyOwner() {
        firstObjectReadMode = FirstObjectReadMode.AnyOwner
    }

    override fun nearestObjectReadBeforeString(value: String) {
        nearestBeforeString = value
    }

    override fun rankBy(label: String, block: RankScope.() -> Int) {
        rankers += label to block
    }

    override fun requireScoreAtLeast(score: Int) {
        minScore = score
    }

    fun resolveFromAnchor(debug: String?): FieldHandle {
        val owner = ownerDescriptor ?: defaultOwner
            ?: error("Field locator '$debug' requires owner(...) for anchor-based lookup")
        val ownerClass = state.classFor(owner) ?: error("Class not found: $owner")
        nearestBeforeString?.let { needle ->
            var best: Pair<FieldHandle, Int>? = null
            for (method in ownerClass.methods) {
                val anchor = method.indexOfFirstString(needle) ?: continue
                for (index in (anchor - 1) downTo 0) {
                    val insn = method.instructions[index]
                    if (insn !is Instruction.RegField || insn.opcode() != Opcodes.IGET_OBJECT) continue
                    val field = insn.value0.field
                    if (field.definingClass != owner) continue
                    val handle = state.wrapField(field, debug = debug ?: "${field.definingClass}:${field.name}")
                    val distance = anchor - index
                    if (best == null || distance < best.second) {
                        best = handle to distance
                    }
                    break
                }
            }
            return best?.first ?: error(nearestObjectReadFailure(owner, needle))
        }

        if (firstObjectReadMode != FirstObjectReadMode.None) {
            val fieldOwner = if (firstObjectReadMode == FirstObjectReadMode.DeclaredOnOwner) owner else null
            for (method in ownerClass.methods) {
                for (insn in method.instructions) {
                    if (insn !is Instruction.RegField || insn.opcode() != Opcodes.IGET_OBJECT) continue
                    val field = insn.value0.field
                    if (fieldOwner != null && field.definingClass != fieldOwner) continue
                    return state.wrapField(field, debug = debug ?: "${field.definingClass}:${field.name}")
                }
            }
            error(firstObjectReadFailure(owner, anyOwner = firstObjectReadMode == FirstObjectReadMode.AnyOwner))
        }

        return resolveOnType(debug ?: owner)
    }

    fun resolveOnType(debug: String): FieldHandle {
        val owner = ownerDescriptor ?: defaultOwner ?: error("No owner type is available for field '$debug'")
        val ownerClass = state.classFor(owner) ?: error("Class not found: $owner")
        val candidates = ownerClass.instanceFields.map { field ->
            val score = rankers.sumOf { (_, ranker) ->
                ranker(RankScopeImpl(state, typeDescriptor = field.fieldType))
            }
            state.wrapField(FieldRef(owner, field.name, field.fieldType), debug = debug) to score
        }
        require(candidates.isNotEmpty()) { "No instance fields found on $owner" }
        val winner = candidates.maxByOrNull { it.second } ?: error("No field candidates on $owner")
        require(winner.second >= minScore) {
            "Best field candidate for '$debug' on $owner scored ${winner.second}, below required $minScore"
        }
        if (rankers.isEmpty()) {
            require(candidates.size == 1) {
                "Field '$debug' on $owner is ambiguous without ranking; found ${candidates.size} candidates"
            }
        }
        return winner.first
    }

    private fun firstObjectReadFailure(owner: String, anyOwner: Boolean): String =
        if (anyOwner) {
            "No object field read found while scanning methods on $owner."
        } else {
            "No object field read found on $owner. " +
                "firstObjectRead() only matches IGET_OBJECT instructions whose field is declared on $owner. " +
                "If you want the first field read in a specific method, use fromMethod(...) + raw { firstFieldRead() } instead. " +
                "If you want the first object field read across methods on $owner regardless of field owner, use firstObjectReadAnyOwner()."
        }

    private fun nearestObjectReadFailure(owner: String, needle: String): String =
        "No object field read before '$needle' on $owner. " +
            "nearestObjectReadBeforeString() searches methods on $owner for an IGET_OBJECT before the anchor string whose field is declared on $owner. " +
            "If you want method-local instruction order, use fromMethod(...) + raw { firstFieldRead() / nextFieldRead(owner = null) }."
}

private enum class FirstObjectReadMode {
    None,
    DeclaredOnOwner,
    AnyOwner,
}

internal class StubBindingCompiler(
    private val state: PatchApiState,
    owner: String,
) : StubBinding {
    private val ownerDescriptor = normalizeDescriptor(owner)
    private val ownerClass = state.classFor(ownerDescriptor)
        ?: error("Stub owner class not found: $ownerDescriptor")

    override fun method(name: String, proto: String, block: CodeScope.() -> Unit) {
        val target = ownerClass.methods.firstOrNull { it.info.methodName == name && it.info.proto == proto }
            ?: error("Stub method not found: $ownerDescriptor->$name$proto")
        target.replaceWithCode(block)
    }
}

private data class FieldReadResult(
    val index: Int,
    val owner: String,
    val type: String,
    val handle: FieldHandle,
)

private data class MethodReadResult(
    val index: Int,
    val ref: MethodRef,
)

private fun normalizeDescriptor(value: String): String =
    if (value.startsWith("L") && value.endsWith(";")) {
        value
    } else {
        "L${value.replace('.', '/')};"
    }

private fun isReferenceType(type: String): Boolean =
    type.startsWith("L") || type.startsWith("[")

private fun ValueRef.asImpl(): ValueRefImpl =
    this as? ValueRefImpl ?: error("ValueRef is only valid inside CodeScope")
