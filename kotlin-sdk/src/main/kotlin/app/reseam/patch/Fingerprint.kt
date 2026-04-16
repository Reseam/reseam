// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class FingerprintMatchContext(val info: MethodInfo, val methodHandle: UInt) {
    val accessFlags: UInt get() = info.accessFlags
    val definingClass: String get() = info.classDescriptor
    val methodDefiningClass: String get() = info.classDescriptor
    val methodName: String get() = info.methodName
    val returnType: String get() = info.returnType
    val parameterTypes: List<String> get() = info.parameterTypes
    val proto: String get() = info.proto
    val registerCount: UShort get() = info.registerCount
    val insSize: UShort get() = info.insSize
    val outsSize: UShort get() = info.outsSize

    val method: Method by lazy { Method(methodHandle) }
    val instructions: List<Instruction> by lazy { method.instructions }

    val immutableClassDef: DexClass by lazy {
        DexClass(findClass(definingClass) ?: error("class not found: $definingClass"))
    }
}

class Fingerprint(
    private val def: FingerprintDef,
    private val customFilter: (FingerprintMatchContext.() -> Boolean)?,
    internal val instructionPredicates: List<IndexedMatcherPredicate<Instruction>>? = null,
) {
    private var result: FingerprintResult? = null
    private var resolved = false
    private var cachedIndices: List<Int>? = null

    val method: Method
        get() {
            resolve()
            return Method(result?.method ?: error("fingerprint did not match: ${def.name ?: "anonymous"}"))
        }

    val classDef: DexClass
        get() = method.classDef

    val immutableMethod: MethodInfo
        get() = method.info

    val matchedCount: Int
        get() {
            resolve()
            return result?.matchedCount?.toInt() ?: 0
        }

    val matched: Boolean
        get() {
            resolve()
            return result != null
        }

    operator fun get(index: Int): Int {
        resolve()
        val indices = matchedInstructionIndices
        return if (index >= 0) indices[index] else indices[indices.size + index]
    }

    val matchedInstructionIndices: List<Int>
        get() {
            resolve()
            if (cachedIndices == null && instructionPredicates != null) {
                cachedIndices = matchInstructionSequence(method.instructions, instructionPredicates)
                    ?: error("instruction predicates did not match in resolved method")
            }
            return cachedIndices ?: emptyList()
        }

    fun findAll(): List<MatchResult> {
        val results = findMethodsByFingerprint(def)
        val filtered = if (customFilter == null) results else results.filter { r ->
            val info = getMethodInfo(r.method) ?: return@filter false
            val ctx = FingerprintMatchContext(info, r.method)
            customFilter.invoke(ctx)
        }
        return filtered.map { MatchResult(it) }
    }

    private fun resolve() {
        if (resolved) return
        resolved = true
        if (customFilter == null) {
            result = findMethodByFingerprint(def)
        } else {
            result = findAll().firstOrNull()?.raw
        }
    }
}

class MatchResult(internal val raw: FingerprintResult) {
    val method: Method get() = Method(raw.method)
    val matchedCount: Int get() = raw.matchedCount.toInt()
}

class FingerprintBuilder {
    private var name: String? = null
    private var definingClass: String? = null
    private var accessFlags: Int? = null
    private var returnType: String? = null
    private var parameters: List<String>? = null
    private var opcodes: List<Int?>? = null
    private var strings: List<String>? = null
    private var literals: MutableList<Long>? = null
    private var customFilter: (FingerprintMatchContext.() -> Boolean)? = null
    internal var instructionPredicates: List<IndexedMatcherPredicate<Instruction>>? = null

    fun name(name: String) { this.name = name }
    fun definingClass(descriptor: String) { this.definingClass = descriptor }
    fun accessFlags(flags: Int) { this.accessFlags = flags }
    fun returnType(type: String) { this.returnType = type }
    fun parameters(vararg params: String) { this.parameters = params.toList() }
    fun parameterTypes(vararg params: String) { this.parameters = params.toList() }
    fun opcodes(vararg ops: Int?) { this.opcodes = ops.toList() }
    fun strings(vararg strs: String) { this.strings = strs.toList() }
    fun literal(value: Long) { (literals ?: mutableListOf<Long>().also { literals = it }).add(value) }
    fun literal(provider: () -> Long) { literal(provider()) }

    fun custom(filter: FingerprintMatchContext.() -> Boolean) {
        val prev = this.customFilter
        this.customFilter = if (prev != null) {
            { prev() && filter() }
        } else {
            filter
        }
    }

    fun instructions(vararg predicates: IndexedMatcherPredicate<Instruction>) {
        instructionPredicates = predicates.toList()
        custom {
            matchInstructionSequence(instructions, predicates.toList()) != null
        }
    }

    @JvmName("instructionsFromArray")
    fun instructions(predicates: Array<out IndexedMatcherPredicate<Instruction>>) {
        instructions(*predicates)
    }

    fun build(): Fingerprint = Fingerprint(
        FingerprintDef(
            name = name,
            definingClass = definingClass,
            accessFlags = accessFlags?.toUInt(),
            returnType = returnType,
            parameters = parameters,
            opcodes = opcodes?.map { it ?: -1 }?.toIntArray(),
            strings = strings,
            literals = literals?.toLongArray(),
        ),
        customFilter,
        instructionPredicates,
    )
}

fun fingerprint(block: FingerprintBuilder.() -> Unit): Fingerprint =
    FingerprintBuilder().apply(block).build()

fun DexClass.fingerprint(block: FingerprintBuilder.() -> Unit): Fingerprint {
    val descriptor = info.descriptor
    return fingerprint {
        definingClass(descriptor)
        block()
    }
}

