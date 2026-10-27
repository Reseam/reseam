// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.allMethodHandles
import app.reseam.patch.native.classMethodsByName
import app.reseam.patch.native.countFollowingCheckCast
import app.reseam.patch.native.fieldReferences
import app.reseam.patch.native.findCallsMatching
import app.reseam.patch.native.findClass
import app.reseam.patch.native.findClassesExtending
import app.reseam.patch.native.findClassesImplementing
import app.reseam.patch.native.findClassesWithInstanceField
import app.reseam.patch.native.findFieldAccessesMatching
import app.reseam.patch.native.findInstructionsByLiteral
import app.reseam.patch.native.findMethodsByName
import app.reseam.patch.native.findMethodsByOpcodes
import app.reseam.patch.native.findMethodsByProto
import app.reseam.patch.native.findMethodsByStringPrefix
import app.reseam.patch.native.findMethodsByStrings
import app.reseam.patch.native.getAllClasses
import app.reseam.patch.native.indexOfFirstStringWithPrefix
import app.reseam.patch.native.isAddedDex
import app.reseam.patch.native.methodReferences
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.InstructionHit
import app.reseam.patch.types.MethodRef

private data class CastQuery(val callee: MethodSignature, val type: String, val lookAhead: Int)

private data class ClassMethods(val owner: String, val name: String?)

private sealed interface Seed {
    data class Name(val value: String) : Seed

    data class Returns(val type: String) : Seed

    data class Parameters(val types: List<String>) : Seed

    data class Parameter(val type: String) : Seed

    data class Op(val value: Opcode) : Seed

    data class Ops(val values: List<Opcode?>) : Seed

    data class Strings(val values: List<String>) : Seed

    data class Prefix(val value: String) : Seed

    data class Literal(val value: Long) : Seed
}

internal class SearchIndex(private val cache: RunCache) {
    private val memos = mutableListOf<Memo<*, *>>()

    /**
     * Results the index keeps until the run's bytecode changes. A full change drops every memo; a
     * method edit applies [onEdit], which drops what the edited methods' code made stale.
     */
    private inner class Memo<K, V>(private val onEdit: MutableMap<K, V>.(UIntArray) -> Unit = {}) {
        private val entries = HashMap<K, V>()

        init {
            memos += this
        }

        operator fun get(key: K): V? {
            cache.synchronize()
            return entries[key]
        }

        fun getOrPut(key: K, compute: () -> V): V {
            cache.synchronize()
            return entries.getOrPut(key, compute)
        }

        fun clear() = entries.clear()

        fun edited(handles: UIntArray) = entries.onEdit(handles)
    }

    private fun <K, V> clearedOnEdit() = Memo<K, V> { clear() }

    private val classList = Memo<Unit, List<DexClass>>()
    private val methodList = Memo<Unit, List<Method>>()
    private val classesBySourceFile = Memo<Unit, Map<String?, List<DexClass>>>()
    private val classByDescriptor = Memo<String, DexClass?>()
    private val methodBySignature = Memo<MethodSignature, Method?>()
    private val methodsByClass = Memo<ClassMethods, List<Method>>()
    private val inheritedByClass = Memo<ClassMethods, List<Method>>()
    private val declarationSeeds = Memo<Seed, Set<UInt>>()
    private val codeSeeds = clearedOnEdit<Seed, Set<UInt>>()
    private val classesByString = clearedOnEdit<String, Set<DexClass>>()
    private val classesByField = Memo<String, Set<DexClass>>()
    private val subclasses = Memo<String, Set<DexClass>>()
    private val implementers = Memo<String, Set<DexClass>>()
    private val enumsByClass = clearedOnEdit<UInt, Map<String, FieldRef>>()
    private val refsByMethod = Memo<UInt, List<MethodRef>> { it.forEach(::remove) }
    private val fieldRefsByMethod = Memo<UInt, List<FieldRef>> { it.forEach(::remove) }
    private val invokeSites = clearedOnEdit<MethodSignature, List<Pair<UInt, Int>>>()
    private val castCounts = clearedOnEdit<CastQuery, Int>()
    private val addedDex = Memo<Int, Boolean>()
    private val matchingCalls = clearedOnEdit<MethodRefMatchSpec, Set<UInt>>()
    private val matchingAccesses = clearedOnEdit<FieldRefMatchSpec, Set<UInt>>()

    fun invalidate() = memos.forEach { it.clear() }

    fun invalidateMethods(handles: UIntArray) = memos.forEach { it.edited(handles) }

    val allClasses: List<DexClass>
        get() = classList.getOrPut(Unit) { getAllClasses().map(::DexClass) }

    val allMethods: List<Method>
        get() = methodList.getOrPut(Unit) { allMethodHandles().map(::Method) }

    /** True for a DEX the patcher linked in, which is every extension the bundle ships. */
    fun isExtensionDex(dexIndex: Int): Boolean =
        addedDex.getOrPut(dexIndex) { isAddedDex(dexIndex.toUInt()) }

    fun classFor(descriptor: String): DexClass? =
        classByDescriptor.getOrPut(descriptor) { findClass(descriptor)?.let(::DexClass) }

    fun requireClass(descriptor: String): DexClass =
        classFor(descriptor) ?: error("Class not found: $descriptor")

    fun methodFor(ref: MethodRef): Method? =
        methodFor(MethodSignature(ref.definingClass, ref.name, ref.proto))

    /**
     * The method a reference names, resolved the way the runtime resolves it: the class's own
     * declaration, else the nearest one it inherits.
     */
    fun methodFor(signature: MethodSignature): Method? =
        methodBySignature.getOrPut(signature) {
            val named = { methods: List<Method> ->
                methods.firstOrNull { it.name == signature.name && it.proto == signature.proto }
            }
            named(methodsInClass(signature.owner, signature.name))
                ?: named(inheritedMethods(signature.owner, signature.name))
        }

    fun methodsInClass(descriptor: String, name: String? = null): List<Method> =
        methodsByClass.getOrPut(ClassMethods(descriptor, name)) {
            classFor(descriptor)
                ?.let { classDef ->
                    if (name == null) classDef.methods
                    else classMethodsByName(classDef.handle, name).map(::Method)
                }
                .orEmpty()
        }

    /**
     * Methods this class inherits from the app classes it extends, nearest first, minus the ones it
     * or a nearer ancestor overrides. Constructors and private declarations are not inherited.
     */
    fun inheritedMethods(descriptor: String, name: String? = null): List<Method> =
        inheritedByClass.getOrPut(ClassMethods(descriptor, name)) {
            val seen =
                methodsInClass(descriptor, name).mapTo(mutableSetOf()) { it.name to it.proto }
            classFor(descriptor)?.superclassChain.orEmpty().flatMap { ancestor ->
                methodsInClass(ancestor.descriptor, name).filter {
                    it.name != "<init>" &&
                        !AccessFlags.PRIVATE.isSet(it.info.accessFlags) &&
                        seen.add(it.name to it.proto)
                }
            }
        }

    private fun seed(memo: Memo<Seed, Set<UInt>>, key: Seed, compute: () -> UIntArray) =
        memo.getOrPut(key) { compute().toCollection(linkedSetOf()) }

    fun methodsWithName(name: String) =
        seed(declarationSeeds, Seed.Name(name)) { findMethodsByName(name) }

    fun methodsWithReturnType(type: String) =
        seed(declarationSeeds, Seed.Returns(type)) { findMethodsByProto(type, null, null) }

    fun methodsWithExactParameters(types: List<String>) =
        seed(declarationSeeds, Seed.Parameters(types.toList())) {
            findMethodsByProto(null, types, null)
        }

    fun methodsWithParameter(type: String) =
        seed(declarationSeeds, Seed.Parameter(type)) { findMethodsByProto(null, null, type) }

    fun methodsWithOpcode(opcode: Opcode) =
        seed(codeSeeds, Seed.Op(opcode)) { findMethodsByOpcodes(intArrayOf(opcode.value)) }

    fun methodsWithOpcodeSequence(opcodes: List<Opcode?>) =
        seed(codeSeeds, Seed.Ops(opcodes.toList())) { findMethodsByOpcodes(opcodes.toPattern()) }

    fun methodsWithStrings(values: List<String>): Set<UInt> {
        val key = values.distinct().sorted()
        return seed(codeSeeds, Seed.Strings(key)) { findMethodsByStrings(key) }
    }

    fun methodsWithStringPrefix(prefix: String) =
        seed(codeSeeds, Seed.Prefix(prefix)) { findMethodsByStringPrefix(prefix) }

    fun methodsWithLiteral(value: Long) =
        seed(codeSeeds, Seed.Literal(value)) {
            findInstructionsByLiteral(value).map { it.method }.toUIntArray()
        }

    fun classesWithString(value: String): Set<DexClass> =
        classesByString.getOrPut(value) {
            methodsWithStrings(listOf(value))
                .asSequence()
                .map { Method(it).owner }
                .distinct()
                .mapNotNull(::classFor)
                .toCollection(linkedSetOf())
        }

    /** An enum's constants by the name each passes to its constructor; empty for other classes. */
    fun enumConstants(classDef: DexClass): Map<String, FieldRef> =
        enumsByClass.getOrPut(classDef.handle) { readEnumConstants(classDef, this) }

    fun classesWithSourceFile(name: String): List<DexClass> =
        classesBySourceFile
            .getOrPut(Unit) {
                cache.prefetchClasses(allClasses.map { it.handle })
                allClasses.groupBy { it.sourceFile }
            }[name]
            .orEmpty()

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        classesBy(classesByField, type, ::findClassesWithInstanceField)

    fun classesExtending(type: String): Set<DexClass> =
        classesBy(subclasses, type, ::findClassesExtending)

    fun classesImplementing(type: String): Set<DexClass> =
        classesBy(implementers, type, ::findClassesImplementing)

    private fun classesBy(
        memo: Memo<String, Set<DexClass>>,
        type: String,
        find: (String) -> UIntArray,
    ): Set<DexClass> =
        memo.getOrPut(type) { find(type).map(::DexClass).toCollection(linkedSetOf()) }

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
                        Method(hit.method).methodRef(hit.index.toInt())?.let(spec::matches) == true
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

    fun methodHasString(method: Method, value: String): Boolean =
        method.indexOfFirstString(value) != null

    fun methodHasOpcode(method: Method, opcode: Opcode): Boolean =
        method.indexOfFirst(opcode) != null

    fun classHasString(classDef: DexClass, value: String): Boolean =
        classDef.methods.any { methodHasString(it, value) }

    fun classExtends(classDef: DexClass, type: String): Boolean =
        classDef.superclass == type || classDef.superclassChain.any { it.descriptor == type }

    fun methodRefsOf(method: Method): List<MethodRef> =
        refsByMethod.getOrPut(method.handle) {
            methodReferences(method.handle).distinct()
        }

    fun fieldRefsOf(method: Method): List<FieldRef> =
        fieldRefsByMethod.getOrPut(method.handle) { fieldReferences(method.handle) }

    fun calleesOf(method: Method): List<Method> = methodRefsOf(method).mapNotNull(::methodFor)

    fun methodCalls(method: Method, target: Method): Boolean =
        methodCalls(method, signatureOf(target))

    fun methodCalls(method: Method, ref: MethodRef): Boolean =
        methodCalls(method, MethodSignature(ref.definingClass, ref.name, ref.proto))

    private fun methodCalls(method: Method, signature: MethodSignature): Boolean =
        invokeSites[signature]?.any { it.first == method.handle }
            ?: methodRefsOf(method).any {
                MethodSignature(it.definingClass, it.name, it.proto) == signature
            }

    fun methodHasStringPrefix(method: Method, prefix: String): Boolean =
        codeSeeds[Seed.Prefix(prefix)]?.contains(method.handle)
            ?: (indexOfFirstStringWithPrefix(method.handle, prefix) != null)

    fun methodHasLiteral(method: Method, value: Long): Boolean =
        codeSeeds[Seed.Literal(value)]?.contains(method.handle) ?: method.containsLiteral(value)

    fun followedByCheckCast(target: Method, type: String, lookAhead: Int): Int {
        val signature = signatureOf(target)
        return castCounts.getOrPut(CastQuery(signature, type, lookAhead)) {
            countFollowingCheckCast(
                    invokeSitesFor(signature).map { (handle, index) ->
                        InstructionHit(handle, index.toUInt())
                    },
                    type,
                    lookAhead.coerceAtLeast(0).toUInt(),
                )
                .toInt()
        }
    }

    /** Candidates from the plan's seeds in order, until no more than one batch remains. */
    fun <T> narrowedPool(plan: QueryPlan<T>): CandidatePool<T> {
        if (plan.seeds.isEmpty()) return plan.unconstrained()
        var current: Set<UInt>? = null
        val pipeline = mutableListOf<String>()
        for (seed in plan.seeds) {
            val seeded = seed()
            pipeline += "${seeded.label}: ${seeded.handles.size} candidate ${plan.unit}"
            val next = current?.intersect(seeded.handles) ?: seeded.handles
            current = next
            if (next.size <= CANDIDATE_BATCH) break
        }
        val winners = current.orEmpty()
        return CandidatePool(plan.wrap(winners), pipeline, winners.size, emptyList(), null)
    }

    /** Candidates from every seed, so a query that matched nothing reports what emptied them. */
    fun <T> exhaustivePool(plan: QueryPlan<T>): CandidatePool<T> {
        if (plan.seeds.isEmpty()) return plan.unconstrained()
        var current: Set<UInt>? = null
        var lastNonEmpty: Set<UInt>? = null
        var exhaustedBy: String? = null
        val pipeline = mutableListOf<String>()
        for (seeded in plan.seeds.map { it() }.sortedBy { it.handles.size }) {
            val next = current?.intersect(seeded.handles) ?: seeded.handles
            pipeline += "${seeded.label}: ${seeded.handles.size} candidate ${plan.unit}"
            if (next.isEmpty()) {
                exhaustedBy = seeded.label
                break
            }
            current = next
            lastNonEmpty = next
        }
        val winners = current.orEmpty()
        val nearMiss = winners.ifEmpty { lastNonEmpty.orEmpty() }
        val considered =
            when {
                winners.isNotEmpty() -> winners.size
                exhaustedBy != null && lastNonEmpty == null -> 0
                else -> nearMiss.size
            }
        return CandidatePool(
            plan.wrap(winners),
            pipeline,
            considered,
            plan.wrap(nearMiss),
            exhaustedBy,
        )
    }

    private fun signatureOf(method: Method) =
        MethodSignature(method.owner, method.name, method.proto)

    fun invokeSitesFor(signature: MethodSignature): List<Pair<UInt, Int>> =
        invokeSites.getOrPut(signature) {
            val ref = MethodRef(signature.owner, signature.name, signature.proto)
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
}
