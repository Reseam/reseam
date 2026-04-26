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

internal class PatchSearchIndex(ctx: PatchRuntime) {
    val allClasses: List<DexClass> = ctx.bytecode.classes
    val allMethods: List<Method> = allMethodHandles().map { Method(it.toUInt()) }

    private val classByDescriptor: Map<String, DexClass> = allClasses.associateBy { it.info.descriptor }
    private val methodByHandle: Map<UInt, Method> = allMethods.associateBy { it.handle }
    private val methodBySignature: Map<MethodSignature, Method> = allMethods.associateBy(::signatureOf)

    private val methodsByClass = mutableMapOf<String, MutableSet<UInt>>()
    private val methodsByReturnType = mutableMapOf<String, MutableSet<UInt>>()
    private val methodsByExactParameters = mutableMapOf<List<String>, MutableSet<UInt>>()
    private val methodsByParameterType = mutableMapOf<String, MutableSet<UInt>>()
    private val methodsByOpcode = mutableMapOf<Int, MutableSet<UInt>>()
    private val methodsByString = mutableMapOf<String, MutableSet<UInt>>()
    private val methodsByLiteral = mutableMapOf<Long, MutableSet<UInt>>()
    private val classesByString = mutableMapOf<String, MutableSet<String>>()
    private val classesByInstanceFieldType = mutableMapOf<String, MutableSet<String>>()
    private val callersByTarget = mutableMapOf<MethodSignature, MutableSet<UInt>>()
    private val invokeSitesByTarget = mutableMapOf<MethodSignature, MutableList<InvokeSite>>()
    private val calleesByMethod = mutableMapOf<UInt, MutableSet<MethodSignature>>()
    private val followedByCastCache = mutableMapOf<CallSiteCastKey, Int>()

    init {
        for (cls in allClasses) {
            val descriptor = cls.info.descriptor
            for (field in cls.instanceFields) {
                classesByInstanceFieldType.getOrPut(field.fieldType) { linkedSetOf() }.add(descriptor)
            }
        }

        for (method in allMethods) {
            val info = method.info
            val handle = method.handle
            val descriptor = info.classDescriptor
            val params = info.parameterTypes

            methodsByClass.getOrPut(descriptor) { linkedSetOf() }.add(handle)
            methodsByReturnType.getOrPut(info.returnType) { linkedSetOf() }.add(handle)
            methodsByExactParameters.getOrPut(params) { linkedSetOf() }.add(handle)
            params.distinct().forEach { param ->
                methodsByParameterType.getOrPut(param) { linkedSetOf() }.add(handle)
            }

            val seenOpcodes = hashSetOf<Int>()
            val seenStrings = linkedSetOf<String>()
            val seenLiterals = linkedSetOf<Long>()
            val seenCallees = linkedSetOf<MethodSignature>()

            method.instructions.forEachIndexed { index, instruction ->
                val opcode = instruction.opcode()
                if (opcode >= 0 && seenOpcodes.add(opcode)) {
                    methodsByOpcode.getOrPut(opcode) { linkedSetOf() }.add(handle)
                }

                instruction.stringValue()?.let { value ->
                    if (seenStrings.add(value)) {
                        methodsByString.getOrPut(value) { linkedSetOf() }.add(handle)
                        classesByString.getOrPut(value) { linkedSetOf() }.add(descriptor)
                    }
                }

                if (instruction is Instruction.RegLiteral) {
                    val literal = instruction.value0.literal
                    if (seenLiterals.add(literal)) {
                        methodsByLiteral.getOrPut(literal) { linkedSetOf() }.add(handle)
                    }
                }

                instruction.methodRef()?.let { ref ->
                    val signature = MethodSignature(ref.definingClass, ref.name, ref.proto)
                    callersByTarget.getOrPut(signature) { linkedSetOf() }.add(handle)
                    invokeSitesByTarget.getOrPut(signature) { mutableListOf() }.add(InvokeSite(handle, index))
                    seenCallees.add(signature)
                }
            }

            if (seenCallees.isNotEmpty()) {
                calleesByMethod[handle] = seenCallees
            }
        }
    }

    fun classFor(descriptor: String): DexClass? =
        classByDescriptor[descriptor]

    fun methodFor(signature: MethodSignature): Method? =
        methodBySignature[signature]

    fun methodsInClass(descriptor: String): List<Method> =
        methodsByClass[descriptor].orEmpty().mapNotNull(methodByHandle::get)

    fun methodsWithReturnType(type: String): Set<UInt> =
        methodsByReturnType[type].orEmpty()

    fun methodsWithExactParameters(types: List<String>): Set<UInt> =
        methodsByExactParameters[types].orEmpty()

    fun methodsWithParameter(type: String): Set<UInt> =
        methodsByParameterType[type].orEmpty()

    fun methodsWithOpcode(opcode: Int): Set<UInt> =
        methodsByOpcode[opcode].orEmpty()

    fun methodsWithString(value: String): Set<UInt> =
        methodsByString[value].orEmpty()

    fun methodsWithLiteral(value: Long): Set<UInt> =
        methodsByLiteral[value].orEmpty()

    fun classesWithString(value: String): Set<DexClass> =
        classesByString[value].orEmpty().mapNotNull(classByDescriptor::get).toSet()

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        classesByInstanceFieldType[type].orEmpty().mapNotNull(classByDescriptor::get).toSet()

    fun callersOf(target: Method): List<Method> =
        callersByTarget[signatureOf(target)].orEmpty().mapNotNull(methodByHandle::get)

    fun calleesOf(target: Method): List<Method> =
        calleesByMethod[target.handle].orEmpty().mapNotNull(methodBySignature::get)

    fun methodHasOpcode(method: Method, opcode: Int): Boolean =
        method.handle in methodsByOpcode[opcode].orEmpty()

    fun followedByCheckCast(target: Method, type: String, lookAhead: Int): Int {
        val key = CallSiteCastKey(signatureOf(target), type, lookAhead)
        return followedByCastCache.getOrPut(key) {
            val sites = invokeSitesByTarget[signatureOf(target)].orEmpty()
            sites.count { site ->
                val caller = methodByHandle[site.methodHandle] ?: return@count false
                val start = site.index + 1
                val end = minOf(caller.instructionCount, start + lookAhead)
                (start until end).any { index ->
                    val instruction = caller.instructions[index]
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
        spec.stringValues.forEach { value ->
            seeds += MethodSeed("strings(\"$value\")", methodsWithString(value))
        }
        spec.literalValues.forEach { value ->
            seeds += MethodSeed("literals($value)", methodsWithLiteral(value))
        }
        spec.returnType?.let { type ->
            seeds += MethodSeed("returnType($type)", methodsWithReturnType(type))
        }
        spec.parameterTypes?.let { types ->
            seeds += MethodSeed("parameterTypes(${types.joinToString()})", methodsWithExactParameters(types))
        }
        spec.hasParameters.forEach { type ->
            seeds += MethodSeed("hasParameter($type)", methodsWithParameter(type))
        }
        spec.opcodes.forEach { opcode ->
            val opcodeValue = OPCODE_NAMES[opcode]
                ?: error("Unknown opcode name '$opcode'")
            seeds += MethodSeed("hasOpcode($opcode)", methodsWithOpcode(opcodeValue))
        }

        if (seeds.isEmpty()) {
            return MethodCandidatePool(
                candidates = allMethods,
                pipeline = listOf("no indexed constraints; considering all ${allMethods.size} methods"),
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
            seeds += ClassSeed("strings(\"$value\")", classesWithString(value))
        }
        spec.instanceFieldTypes.forEach { type ->
            seeds += ClassSeed("hasInstanceField($type)", classesWithInstanceFieldType(type))
        }

        if (seeds.isEmpty()) {
            return ClassCandidatePool(
                candidates = allClasses,
                pipeline = listOf("no indexed constraints; considering all ${allClasses.size} classes"),
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
            pipeline += "${seed.label}: ${seed.candidates.size} indexed candidates"
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
            else -> allMethods.map(Method::handle).toSet()
        }

        return MethodCandidatePool(
            candidates = winnerSeed.mapNotNull(methodByHandle::get),
            pipeline = pipeline,
            considered = when {
                winnerSeed.isNotEmpty() -> winnerSeed.size
                exhaustedBy != null && lastNonEmpty == null -> 0
                else -> nearMissSeed.size
            },
            nearMissSeed = nearMissSeed.mapNotNull(methodByHandle::get),
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
            pipeline += "${seed.label}: ${seed.candidates.size} indexed candidates"
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
            else -> allClasses.toSet()
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
