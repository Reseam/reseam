// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

internal data class MethodSignature(
    val owner: String,
    val name: String,
    val proto: String,
)

internal data class MethodCandidatePool(
    val candidates: List<Method>,
    val pipeline: List<String>,
    val considered: Int,
    val nearMissSeed: List<Method>,
    val exhaustedBy: String?,
)

internal data class ClassCandidatePool(
    val candidates: List<DexClass>,
    val pipeline: List<String>,
    val considered: Int,
    val nearMissSeed: List<DexClass>,
    val exhaustedBy: String?,
)

private data class CallSiteCastKey(
    val method: MethodSignature,
    val type: String,
    val lookAhead: Int,
)

private data class InvokeSite(
    val methodHandle: UInt,
    val index: Int,
)

internal class PatchSearchIndex(private val ctx: PatchRuntime) {
    val allClasses: List<DexClass> by lazy { ctx.bytecode.classes }
    val allMethods: List<Method> by lazy { allMethodHandles().map { Method(it.toUInt()) } }

    private val classByDescriptor = mutableMapOf<String, DexClass?>()
    private val methodBySignature = mutableMapOf<MethodSignature, Method?>()
    private val methodsByClass = mutableMapOf<String, List<Method>>()
    private val methodsByStrings = mutableMapOf<List<String>, Set<UInt>>()
    private val methodsByLiteral = mutableMapOf<Long, Set<UInt>>()
    private val methodsByOpcode = mutableMapOf<Int, Set<UInt>>()
    private val methodsByReturnType = mutableMapOf<String, Set<UInt>>()
    private val methodsByParameterTypes = mutableMapOf<List<String>, Set<UInt>>()
    private val methodsByParameter = mutableMapOf<String, Set<UInt>>()
    private val classesByString = mutableMapOf<String, Set<DexClass>>()
    private val classesByInstanceFieldType = mutableMapOf<String, Set<DexClass>>()
    private val callersByTarget = mutableMapOf<MethodSignature, List<Method>>()
    private val invokeSitesByTarget = mutableMapOf<MethodSignature, List<InvokeSite>>()
    private val calleesByMethod = mutableMapOf<UInt, List<Method>>()
    private val followedByCastCache = mutableMapOf<CallSiteCastKey, Int>()

    fun classFor(descriptor: String): DexClass? =
        classByDescriptor.getOrPut(descriptor) {
            findClass(descriptor)?.let { DexClass(it) }
        }

    fun methodFor(signature: MethodSignature): Method? =
        methodBySignature.getOrPut(signature) {
            methodsInClass(signature.owner).firstOrNull { method ->
                val info = method.info
                info.methodName == signature.name && info.proto == signature.proto
            }
        }

    fun methodsInClass(descriptor: String): List<Method> =
        methodsByClass.getOrPut(descriptor) {
            classFor(descriptor)?.methods.orEmpty()
        }

    fun methodsWithReturnType(type: String): Set<UInt> =
        methodsByReturnType.getOrPut(type) {
            findMethodsByProto(type, null, null).mapTo(linkedSetOf()) { it.toUInt() }
        }

    fun methodsWithExactParameters(types: List<String>): Set<UInt> =
        methodsByParameterTypes.getOrPut(types) {
            findMethodsByProto(null, types, null).mapTo(linkedSetOf()) { it.toUInt() }
        }

    fun methodsWithParameter(type: String): Set<UInt> =
        methodsByParameter.getOrPut(type) {
            findMethodsByProto(null, null, type).mapTo(linkedSetOf()) { it.toUInt() }
        }

    fun methodsWithOpcode(opcode: Int): Set<UInt> =
        methodsByOpcode.getOrPut(opcode) {
            findMethodsByOpcodes(intArrayOf(opcode)).mapTo(linkedSetOf()) { it.toUInt() }
        }

    fun methodsWithString(value: String): Set<UInt> =
        methodsWithStrings(listOf(value))

    fun methodsWithStrings(values: List<String>): Set<UInt> {
        val key = normalizedStrings(values)
        if (key.isEmpty()) return emptySet()
        return methodsByStrings.getOrPut(key) {
            findMethodsByStrings(key).mapTo(linkedSetOf()) { it.toUInt() }
        }
    }

    fun methodsWithLiteral(value: Long): Set<UInt> =
        methodsByLiteral.getOrPut(value) {
            findInstructionsByLiteral(value).mapTo(linkedSetOf()) { it.method }
        }

    fun classesWithString(value: String): Set<DexClass> =
        classesByString.getOrPut(value) {
            methodsWithString(value).asSequence()
                .map { Method(it).info.classDescriptor }
                .distinct()
                .mapNotNull(::classFor)
                .toCollection(linkedSetOf())
        }

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        classesByInstanceFieldType.getOrPut(type) {
            allClasses.asSequence()
                .filter { classHasInstanceFieldType(it, type) }
                .toCollection(linkedSetOf())
        }

    fun methodHasString(method: Method, value: String): Boolean =
        method.indexOfFirstString(value) != null

    fun methodHasLiteral(method: Method, value: Long): Boolean =
        method.containsLiteral(value)

    fun methodHasOpcode(method: Method, opcode: Int): Boolean =
        method.indexOfFirst(opcode) != null

    fun classHasString(classDef: DexClass, value: String): Boolean =
        classDef.methods.any { methodHasString(it, value) }

    fun classHasInstanceFieldType(classDef: DexClass, type: String): Boolean =
        classDef.instanceFields.any { it.fieldType == type }

    fun callersOf(target: Method): List<Method> {
        val signature = signatureOf(target)
        return callersByTarget.getOrPut(signature) {
            invokeSitesFor(signature).asSequence()
                .map { Method(it.methodHandle) }
                .distinctBy { it.handle }
                .sortedBy(::describeMethod)
                .toList()
        }
    }

    fun calleesOf(target: Method): List<Method> =
        calleesByMethod.getOrPut(target.handle) {
            val signatures = linkedSetOf<MethodSignature>()
            for (index in 0 until target.instructionCount) {
                target.methodRef(index)?.let { ref ->
                    signatures += MethodSignature(ref.definingClass, ref.name, ref.proto)
                }
            }
            signatures.mapNotNull(::methodFor)
        }

    fun followedByCheckCast(target: Method, type: String, lookAhead: Int): Int {
        val key = CallSiteCastKey(signatureOf(target), type, lookAhead)
        return followedByCastCache.getOrPut(key) {
            invokeSitesFor(signatureOf(target)).count { site ->
                val caller = Method(site.methodHandle)
                val start = site.index + 1
                val end = minOf(caller.instructionCount, start + lookAhead)
                (start until end).any { index ->
                    val instruction = getInstruction(caller.handle, index.toUInt())
                    instruction.opcode() == Opcodes.CHECK_CAST && instruction.typeRef() == type
                }
            }
        }
    }

    fun initialMethodCandidates(spec: MethodQuerySpec): MethodCandidatePool {
        val seeds = mutableListOf<MethodSeed>()

        spec.sameAs?.let {
            val methods = listOf(it.method)
            return MethodCandidatePool(
                candidates = methods,
                pipeline = listOf("seeded from sameAs(${describeMethod(it.method)})"),
                considered = methods.size,
                nearMissSeed = methods,
                exhaustedBy = null,
            )
        }

        spec.inClass?.let {
            val seeded = methodsInClass(it.descriptor)
            seeds += MethodSeed("inClass(${it.descriptor})", seeded.map(Method::handle).toSet())
        }
        spec.calls.forEach { target ->
            seeds += MethodSeed("calls(${describeMethod(target.method)})", callersOf(target.method).map(Method::handle).toSet())
        }
        spec.calledBy.forEach { caller ->
            seeds += MethodSeed("calledBy(${describeMethod(caller.method)})", calleesOf(caller.method).map(Method::handle).toSet())
        }
        val opcodeValues = spec.opcodes.map { opcode ->
            opcode to (OPCODE_NAMES[opcode] ?: error("Unknown opcode name '$opcode'"))
        }

        if (spec.stringValues.isNotEmpty()) {
            seeds += MethodSeed("strings(${spec.stringValues.joinToString(transform = ::quoted)})", methodsWithStrings(spec.stringValues))
        }
        spec.literalValues.forEach { value ->
            seeds += MethodSeed("literals($value)", methodsWithLiteral(value))
        }

        if (seeds.isEmpty()) {
            spec.returnType?.let { type ->
                seeds += MethodSeed("returnType($type)", methodsWithReturnType(type))
            }
            spec.parameterTypes?.let { types ->
                seeds += MethodSeed("parameterTypes(${types.joinToString()})", methodsWithExactParameters(types))
            }
            spec.hasParameters.forEach { type ->
                seeds += MethodSeed("hasParameter($type)", methodsWithParameter(type))
            }
            opcodeValues.forEach { (opcode, opcodeValue) ->
                seeds += MethodSeed("hasOpcode($opcode)", methodsWithOpcode(opcodeValue))
            }
        }

        if (seeds.isEmpty()) {
            return MethodCandidatePool(
                candidates = allMethods,
                pipeline = listOf("no selective constraints; considering all ${allMethods.size} methods"),
                considered = allMethods.size,
                nearMissSeed = allMethods.take(12),
                exhaustedBy = null,
            )
        }

        return methodPoolFromSeeds(seeds)
    }

    fun initialClassCandidates(spec: ClassQuerySpec): ClassCandidatePool {
        val seeds = mutableListOf<ClassSeed>()
        spec.stringValues.forEach { value ->
            seeds += ClassSeed("strings(${quoted(value)})", classesWithString(value))
        }

        if (seeds.isEmpty()) {
            spec.instanceFieldTypes.forEach { type ->
                seeds += ClassSeed("hasInstanceField($type)", classesWithInstanceFieldType(type))
            }
        }

        if (seeds.isEmpty()) {
            return ClassCandidatePool(
                candidates = allClasses,
                pipeline = listOf("no selective constraints; considering all ${allClasses.size} classes"),
                considered = allClasses.size,
                nearMissSeed = allClasses.take(12),
                exhaustedBy = null,
            )
        }

        return classPoolFromSeeds(seeds)
    }

    private fun methodPoolFromSeeds(seeds: List<MethodSeed>): MethodCandidatePool {
        var current: Set<UInt>? = null
        var lastNonEmpty: Set<UInt>? = null
        var exhaustedBy: String? = null
        val pipeline = mutableListOf<String>()

        for (seed in seeds.sortedBy { it.candidates.size }) {
            val next = current?.intersect(seed.candidates) ?: seed.candidates
            pipeline += "${seed.label}: ${seed.candidates.size} candidate method(s)"
            if (next.isEmpty()) {
                exhaustedBy = seed.label
                break
            }
            current = next
            lastNonEmpty = next
        }

        val winnerSeed = current ?: emptySet()
        val nearMissSeed = when {
            winnerSeed.isNotEmpty() -> winnerSeed
            lastNonEmpty != null -> lastNonEmpty
            else -> emptySet()
        }

        return MethodCandidatePool(
            candidates = winnerSeed.map(::Method),
            pipeline = pipeline,
            considered = when {
                winnerSeed.isNotEmpty() -> winnerSeed.size
                exhaustedBy != null && lastNonEmpty == null -> 0
                else -> nearMissSeed.size
            },
            nearMissSeed = nearMissSeed.map(::Method),
            exhaustedBy = exhaustedBy,
        )
    }

    private fun classPoolFromSeeds(seeds: List<ClassSeed>): ClassCandidatePool {
        var current: Set<DexClass>? = null
        var lastNonEmpty: Set<DexClass>? = null
        var exhaustedBy: String? = null
        val pipeline = mutableListOf<String>()

        for (seed in seeds.sortedBy { it.candidates.size }) {
            val next = current?.intersect(seed.candidates) ?: seed.candidates
            pipeline += "${seed.label}: ${seed.candidates.size} candidate class(es)"
            if (next.isEmpty()) {
                exhaustedBy = seed.label
                break
            }
            current = next
            lastNonEmpty = next
        }

        val winnerSeed = current ?: emptySet()
        val nearMissSeed = when {
            winnerSeed.isNotEmpty() -> winnerSeed
            lastNonEmpty != null -> lastNonEmpty
            else -> emptySet()
        }

        return ClassCandidatePool(
            candidates = winnerSeed.sortedBy { it.info.descriptor },
            pipeline = pipeline,
            considered = when {
                winnerSeed.isNotEmpty() -> winnerSeed.size
                exhaustedBy != null && lastNonEmpty == null -> 0
                else -> nearMissSeed.size
            },
            nearMissSeed = nearMissSeed.sortedBy { it.info.descriptor },
            exhaustedBy = exhaustedBy,
        )
    }

    private fun signatureOf(method: Method): MethodSignature =
        MethodSignature(method.info.classDescriptor, method.info.methodName, method.info.proto)

    private fun invokeSitesFor(signature: MethodSignature): List<InvokeSite> =
        invokeSitesByTarget.getOrPut(signature) {
            findInstructionsByInvoke(signature.owner, signature.name).asSequence()
                .filter { hit ->
                    Method(hit.method).methodRef(hit.index.toInt())?.let { ref ->
                        ref.definingClass == signature.owner &&
                            ref.name == signature.name &&
                            ref.proto == signature.proto
                    } == true
                }
                .map { InvokeSite(it.method, it.index.toInt()) }
                .toList()
        }

    private fun normalizedStrings(values: List<String>): List<String> =
        values.distinct().sorted()

    private fun quoted(value: String): String =
        "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

    private fun describeMethod(method: Method): String =
        "${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"
}

private data class MethodSeed(
    val label: String,
    val candidates: Set<UInt>,
)

private data class ClassSeed(
    val label: String,
    val candidates: Set<DexClass>,
)
