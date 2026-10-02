// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.typeRef
import app.reseam.patch.native.allMethodHandles
import app.reseam.patch.native.classMethodsByName
import app.reseam.patch.native.findCallsMatching
import app.reseam.patch.native.findClass
import app.reseam.patch.native.findClassesWithInstanceField
import app.reseam.patch.native.findInstructionsByLiteral
import app.reseam.patch.native.findInstructionsByStringContains
import app.reseam.patch.native.findMethodsByName
import app.reseam.patch.native.findMethodsByOpcodes
import app.reseam.patch.native.findMethodsByProto
import app.reseam.patch.native.findMethodsByStrings
import app.reseam.patch.native.getAllClasses
import app.reseam.patch.native.getInstruction
import app.reseam.patch.native.isAddedDex
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

internal class SearchIndex(private val runtime: PatchRuntime) {
    private var classes: List<DexClass>? = null
    private var methods: List<Method>? = null
    private var sourceFiles: Map<String?, List<DexClass>>? = null
    val allClasses: List<DexClass>
        get() {
            runtime.synchronize()
            return classes ?: getAllClasses().map(::DexClass).also { classes = it }
        }

    val allMethods: List<Method>
        get() {
            runtime.synchronize()
            return methods ?: allMethodHandles().map(::Method).also { methods = it }
        }

    private val classesBySourceFile: Map<String?, List<DexClass>>
        get() {
            runtime.synchronize()
            return sourceFiles ?: allClasses.groupBy { it.sourceFile }.also { sourceFiles = it }
        }

    private val classByDescriptor = HashMap<String, DexClass?>()
    private val methodBySignature = HashMap<MethodSignature, Method?>()
    private val methodsByClass = HashMap<ClassMethods, List<Method>>()
    private val inheritedByClass = HashMap<ClassMethods, List<Method>>()
    private val seeds = HashMap<Seed, Set<UInt>>()
    private val classesByString = HashMap<String, Set<DexClass>>()
    private val classesByField = HashMap<String, Set<DexClass>>()
    private val refsByMethod = HashMap<UInt, List<MethodRef>>()
    private val invokeSites = HashMap<MethodSignature, List<Pair<UInt, Int>>>()
    private val castCounts = HashMap<CastQuery, Int>()
    private val addedDex = HashMap<Int, Boolean>()

    fun invalidate() {
        classes = null
        methods = null
        sourceFiles = null
        classByDescriptor.clear()
        methodBySignature.clear()
        methodsByClass.clear()
        inheritedByClass.clear()
        seeds.clear()
        classesByString.clear()
        classesByField.clear()
        refsByMethod.clear()
        invokeSites.clear()
        castCounts.clear()
        addedDex.clear()
        matchingCalls.clear()
    }

    private fun <K, V> cached(cache: MutableMap<K, V>, key: K, compute: () -> V): V {
        runtime.synchronize()
        return cache.getOrPut(key, compute)
    }

    /** True for a DEX the patcher linked in, which is every extension the bundle ships. */
    fun isExtensionDex(dexIndex: Int): Boolean =
        cached(addedDex, dexIndex) { isAddedDex(dexIndex.toUInt()) }

    fun classFor(descriptor: String): DexClass? =
        cached(classByDescriptor, descriptor) { findClass(descriptor)?.let(::DexClass) }

    fun methodFor(ref: MethodRef): Method? =
        methodFor(MethodSignature(ref.definingClass, ref.name, ref.proto))

    /**
     * The method a reference names, resolved the way the runtime resolves it: the class's own
     * declaration, else the nearest one it inherits.
     */
    fun methodFor(signature: MethodSignature): Method? =
        cached(methodBySignature, signature) {
            val named = { methods: List<Method> ->
                methods.firstOrNull { it.name == signature.name && it.proto == signature.proto }
            }
            named(methodsInClass(signature.owner, signature.name))
                ?: named(inheritedMethods(signature.owner, signature.name))
        }

    fun methodsInClass(descriptor: String, name: String? = null): List<Method> =
        cached(methodsByClass, ClassMethods(descriptor, name)) {
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
        cached(inheritedByClass, ClassMethods(descriptor, name)) {
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

    private fun seed(key: Seed, compute: () -> UIntArray): Set<UInt> =
        cached(seeds, key) { compute().toCollection(linkedSetOf()) }

    fun methodsWithName(name: String) = seed(Seed.Name(name)) { findMethodsByName(name) }

    fun methodsWithReturnType(type: String) =
        seed(Seed.Returns(type)) { findMethodsByProto(type, null, null) }

    fun methodsWithExactParameters(types: List<String>) =
        seed(Seed.Parameters(types.toList())) { findMethodsByProto(null, types, null) }

    fun methodsWithParameter(type: String) =
        seed(Seed.Parameter(type)) { findMethodsByProto(null, null, type) }

    fun methodsWithOpcode(opcode: Opcode) =
        seed(Seed.Op(opcode)) { findMethodsByOpcodes(intArrayOf(opcode.value)) }

    fun methodsWithOpcodeSequence(opcodes: List<Opcode?>) =
        seed(Seed.Ops(opcodes.toList())) { findMethodsByOpcodes(opcodes.toPattern()) }

    fun methodsWithStrings(values: List<String>): Set<UInt> {
        val key = values.distinct().sorted()
        return seed(Seed.Strings(key)) { findMethodsByStrings(key) }
    }

    fun methodsWithStringPrefix(prefix: String) =
        seed(Seed.Prefix(prefix)) {
            findInstructionsByStringContains(prefix)
                .filter {
                    Method(it.method).stringRef(it.index.toInt())?.startsWith(prefix) == true
                }
                .map { it.method }
                .toUIntArray()
        }

    fun methodsWithLiteral(value: Long) =
        seed(Seed.Literal(value)) {
            findInstructionsByLiteral(value).map { it.method }.toUIntArray()
        }

    fun classesWithString(value: String): Set<DexClass> =
        cached(classesByString, value) {
            methodsWithStrings(listOf(value))
                .asSequence()
                .map { Method(it).owner }
                .distinct()
                .mapNotNull(::classFor)
                .toCollection(linkedSetOf())
        }

    fun classesWithSourceFile(name: String): List<DexClass> = classesBySourceFile[name].orEmpty()

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        cached(classesByField, type) {
            findClassesWithInstanceField(type).map(::DexClass).toCollection(linkedSetOf())
        }

    private val matchingCalls = mutableMapOf<MethodRefMatchSpec, Set<UInt>>()

    fun matchesCall(method: Method, spec: MethodRefMatchSpec): Boolean {
        runtime.synchronize()
        return matchingCalls[spec]?.contains(method.handle)
            ?: methodRefsOf(method).any(spec::matches)
    }

    fun methodsCalling(spec: MethodRefMatchSpec): Set<UInt> =
        cached(matchingCalls, spec) {
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

    fun methodHasString(method: Method, value: String): Boolean =
        method.indexOfFirstString(value) != null

    fun methodHasOpcode(method: Method, opcode: Opcode): Boolean =
        method.indexOfFirst(opcode) != null

    fun classHasString(classDef: DexClass, value: String): Boolean =
        classDef.methods.any { methodHasString(it, value) }

    fun classExtends(classDef: DexClass, type: String): Boolean =
        classDef.superclass == type || classDef.superclassChain.any { it.descriptor == type }

    fun methodRefsOf(method: Method): List<MethodRef> =
        cached(refsByMethod, method.handle) {
            method.instructions.mapNotNull { it.methodRef }.distinct()
        }

    fun calleesOf(method: Method): List<Method> = methodRefsOf(method).mapNotNull(::methodFor)

    fun methodCalls(method: Method, target: Method): Boolean {
        val signature = signatureOf(target)
        return methodRefsOf(method).any {
            MethodSignature(it.definingClass, it.name, it.proto) == signature
        }
    }

    fun methodCalls(method: Method, ref: MethodRef): Boolean =
        invokeSitesFor(MethodSignature(ref.definingClass, ref.name, ref.proto)).any {
            it.first == method.handle
        }

    fun followedByCheckCast(target: Method, type: String, lookAhead: Int): Int {
        val signature = signatureOf(target)
        return cached(castCounts, CastQuery(signature, type, lookAhead)) {
            invokeSitesFor(signature).count { (handle, index) ->
                val caller = Method(handle)
                val end = minOf(caller.instructionCount, index + 1 + lookAhead)
                (index + 1 until end).any { i ->
                    val instruction = getInstruction(handle, i.toUInt())
                    instruction.opcode == Opcode.CHECK_CAST && instruction.typeRef == type
                }
            }
        }
    }

    fun <T> poolFromSeeds(
        seeds: List<Pair<String, Set<UInt>>>,
        unit: String,
        wrap: (Collection<UInt>) -> List<T>,
    ): CandidatePool<T> {
        var current: Set<UInt>? = null
        var lastNonEmpty: Set<UInt>? = null
        var exhaustedBy: String? = null
        val pipeline = mutableListOf<String>()
        for ((label, candidates) in seeds.sortedBy { it.second.size }) {
            val next = current?.intersect(candidates) ?: candidates
            pipeline += "$label: ${candidates.size} candidate $unit"
            if (next.isEmpty()) {
                exhaustedBy = label
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
        return CandidatePool(wrap(winners), pipeline, considered, wrap(nearMiss), exhaustedBy)
    }

    private fun signatureOf(method: Method) =
        MethodSignature(method.owner, method.name, method.proto)

    fun invokeSitesFor(signature: MethodSignature): List<Pair<UInt, Int>> =
        cached(invokeSites, signature) {
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
