// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.allSet
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.methodRef
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.dex.opcode
import app.reseam.patch.dex.typeRef
import app.reseam.patch.native.MethodRef
import app.reseam.patch.native.allMethodHandles
import app.reseam.patch.native.findClass
import app.reseam.patch.native.findCallsMatching
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

interface MethodQuery {
    fun name(value: String)
    fun strings(vararg values: String)
    /** Contains a string literal beginning with [prefix], seeded from the string index. */
    fun stringsStartingWith(prefix: String)
    fun literals(vararg values: Long)
    fun returns(type: String)
    fun params(vararg types: String)
    fun param(index: Int, type: String)
    fun hasParam(type: String)
    fun paramCount(count: Int)
    /** Every access flag the method must carry: `flags(AccessFlags.PUBLIC or AccessFlags.STATIC)` needs both. */
    fun flags(mask: Int)
    /**
     * The method belongs to this class. `inherited` widens that to the methods
     * the class inherits from app classes it extends, which is what a call site
     * on the class resolves to; by default only its own declarations match.
     */
    fun inClass(target: ClassTarget, inherited: Boolean = false)
    fun calls(target: MethodTarget)
    /** Calls this exact reference, including platform methods outside the app. */
    fun calls(ref: MethodRef)
    /** Indexed call-reference query, using the same matcher as instruction points. */
    fun calls(block: MethodRefMatch.() -> Unit)
    fun calledBy(target: MethodTarget)
    /** The method invokes something matching `predicate`, in the app or the platform. */
    fun callsMethod(predicate: MethodRef.() -> Boolean)
    /** The method contains each opcode somewhere, in any order. */
    fun opcode(vararg opcodes: Opcode)
    /**
     * The method contains these opcodes back to back, `null` matching any one
     * instruction. Ordered instruction shape is a last-resort constraint: it
     * seeds only when nothing more selective is in the query.
     */
    fun opcodeSequence(vararg opcodes: Opcode?)
    /**
     * The candidate satisfies `predicate`, for what the rest of the query
     * cannot say. A filter and never a seed, so it needs a selective
     * constraint beside it or the query considers every method in the app.
     */
    fun custom(predicate: Method.() -> Boolean)
    fun rankBy(label: String, block: MethodRankScope.() -> Int)
    /** Take the best candidate when several match instead of failing. */
    fun first()
    /** Also consider classes the bundle's extensions define; a query searches app code only. */
    fun includeExtensions()
}

interface ClassQuery {
    fun strings(vararg values: String)
    fun hasInstanceField(type: String)
    fun extends(type: String)
    fun implements(type: String)
    /** The name the compiler recorded for the class, kept by many obfuscators. */
    fun sourceFile(name: String)
    /**
     * The candidate satisfies `predicate`, for what the rest of the query
     * cannot say. A filter and never a seed, so it needs a selective
     * constraint beside it or the query considers every class in the app.
     */
    fun custom(predicate: DexClass.() -> Boolean)
    fun rankBy(label: String, block: ClassRankScope.() -> Int)
    fun first()
    /** Also consider classes the bundle's extensions define; a query searches app code only. */
    fun includeExtensions()
}

interface RankScope {
    /** The candidate's class descriptor. */
    val type: String
    fun methods(proto: String): List<Method>
    fun zeroArgListGetters(): Int
}

interface MethodRankScope : RankScope {
    val method: Method
    val paramCount: Int
    /** How many call sites of the candidate cast the result to `type` within `lookAhead` instructions. */
    fun callSitesFollowedByCast(type: String, lookAhead: Int = 40): Int
}

interface ClassRankScope : RankScope {
    val classDef: DexClass
}

internal abstract class QuerySpec<T : Any, S : RankScope>(private val kind: String) {
    private val rankers = mutableListOf<Ranker<S>>()
    private var takeFirst = false
    private var searchExtensions = false

    fun rankBy(label: String, block: S.() -> Int) { rankers += Ranker(label, block) }
    fun first() { takeFirst = true }
    fun includeExtensions() { searchExtensions = true }

    /**
     * Excludes extensions by default so lazy linking cannot change query results.
     */
    protected fun isHiddenExtension(dexIndex: Int, index: SearchIndex): Boolean =
        !searchExtensions && index.isExtensionDex(dexIndex)

    protected fun extensionReason(): String? = if (searchExtensions) "including extension classes" else null

    protected abstract fun candidates(runtime: PatchRuntime): CandidatePool<T>
    protected abstract fun mismatch(value: T, runtime: PatchRuntime): String?
    protected abstract fun matchReasons(): List<String>
    protected abstract fun rankScope(index: SearchIndex, value: T): S
    protected abstract fun describe(value: T): String

    fun resolveOne(runtime: PatchRuntime, debugName: String?): Resolution<T> {
        val evaluated = evaluate(runtime)
        val winner = evaluated.accepted.firstOrNull() ?: error(noMatchMessage(kind, evaluated.noMatchReport(debugName)))
        val runnerUp = evaluated.accepted.getOrNull(1)
        check(takeFirst || runnerUp == null || rankers.isNotEmpty() && winner.score > runnerUp.score) {
            "${evaluated.accepted.size} ${kind}s matched '${debugName ?: "anonymous"}'; add constraints, rank them, or take first(): " +
                evaluated.accepted.take(6).joinToString("; ") { "${describe(it.value)} [score=${it.score}]" }
        }
        return Resolution(winner.value, evaluated.report(debugName, winner))
    }

    fun resolveAll(runtime: PatchRuntime, debugName: String?): Resolution<List<T>> {
        val evaluated = evaluate(runtime)
        val report = SearchMatchReport(
            debugName ?: "anonymous",
            "${evaluated.accepted.size} $kind(s)",
            evaluated.considered,
            evaluated.pipeline,
            evaluated.accepted.take(3).map { describe(it.value) },
        )
        return Resolution(evaluated.accepted.map { it.value }, report)
    }

    private fun evaluate(runtime: PatchRuntime): Evaluated<T> {
        val pool = candidates(runtime)
        val scored = mutableListOf<Scored<T>>()
        val rejected = mutableListOf<Rejected<T>>()
        for (value in pool.candidates.ifEmpty { pool.nearMissSeed }) {
            val failure = mismatch(value, runtime)
            if (failure != null) {
                rejected.addBounded(Rejected(value, failure)) { describe(it.value) }
                continue
            }
            val scope = rankScope(runtime.index, value)
            val scores = rankers.map { it.label to it.block(scope) }
            scored += Scored(value, scores.sumOf { it.second }, pool.pipeline + matchReasons() + scores.map { "${it.first}=${it.second}" })
        }
        return Evaluated(
            accepted = scored.sortedWith(compareByDescending<Scored<T>> { it.score }.thenBy { describe(it.value) }),
            considered = pool.considered,
            pipeline = pool.pipeline,
            exhaustedBy = pool.exhaustedBy,
            rejected = rejected.sortedBy { describe(it.value) }.take(3).map { "${describe(it.value)} [missed: ${it.failure}]" },
            describe = ::describe,
        )
    }
}

internal class MethodQuerySpec : QuerySpec<Method, MethodRankScope>("method"), MethodQuery {
    var methodName: String? = null
    val stringValues = mutableListOf<String>()
    val stringPrefixes = mutableListOf<String>()
    val literalValues = mutableListOf<Long>()
    var returnType: String? = null
    var parameterTypes: List<String>? = null
    val positionalParams = mutableMapOf<Int, String>()
    val hasParameters = mutableListOf<String>()
    var parameterCount: Int? = null
    var flagMask = 0
    var inClass: ClassTarget? = null
    var inheritedMethods = false
    val calls = mutableListOf<() -> MethodRef>()
    val calledBy = mutableListOf<MethodTarget>()
    val callMatches = mutableListOf<MethodRefMatchSpec>()
    val callPredicates = mutableListOf<MethodRef.() -> Boolean>()
    val opcodes = mutableListOf<Opcode>()
    val opcodeSequences = mutableListOf<List<Opcode?>>()
    val predicates = mutableListOf<Method.() -> Boolean>()

    override fun name(value: String) { methodName = value }
    override fun strings(vararg values: String) { stringValues += values }
    override fun stringsStartingWith(prefix: String) {
        require(prefix.isNotEmpty()) { "A string prefix must not be empty" }
        stringPrefixes += prefix
    }
    override fun literals(vararg values: Long) { literalValues += values.toList() }
    override fun returns(type: String) { returnType = descriptor(type) }
    override fun params(vararg types: String) { parameterTypes = types.map(::descriptor) }
    override fun param(index: Int, type: String) { positionalParams[index] = descriptor(type) }
    override fun hasParam(type: String) { hasParameters += descriptor(type) }
    override fun paramCount(count: Int) { parameterCount = count }
    override fun flags(mask: Int) { flagMask = flagMask or mask }
    override fun inClass(target: ClassTarget, inherited: Boolean) { inClass = target; inheritedMethods = inherited }
    override fun calls(target: MethodTarget) { calls += { target.ref } }
    override fun calls(ref: MethodRef) { calls += { ref } }
    override fun calls(block: MethodRefMatch.() -> Unit) {
        val spec = MethodRefMatchSpec().apply(block)
        require(spec.owner != null || spec.name != null || spec.returnType != null || spec.parameters != null) {
            "calls requires an owner, name, return type, or parameter signature"
        }
        callMatches += spec
    }
    override fun calledBy(target: MethodTarget) { calledBy += target }
    override fun callsMethod(predicate: MethodRef.() -> Boolean) { callPredicates += predicate }
    override fun opcode(vararg opcodes: Opcode) { this.opcodes += opcodes }
    override fun opcodeSequence(vararg opcodes: Opcode?) { opcodeSequences += opcodes.toList() }
    override fun custom(predicate: Method.() -> Boolean) { predicates += predicate }

    override fun candidates(runtime: PatchRuntime) = runtime.index.methodCandidates(this, runtime)
    override fun rankScope(index: SearchIndex, value: Method) = MethodRankScopeImpl(index, value)
    override fun describe(value: Method) = value.descriptor

    override fun mismatch(value: Method, runtime: PatchRuntime): String? {
        val index = runtime.index
        val info = value.info
        // `inClass` names the owner, so the search never wandered into extension code and the
        // methods of an extension class stay reachable through its own target.
        if (inClass == null && isHiddenExtension(info.dexIndex.toInt(), index)) return "defined by a bundle extension"
        methodName?.let { if (info.methodName != it) return "name mismatch" }
        if (stringValues.any { !index.methodHasString(value, it) }) return "missing required string"
        if (stringPrefixes.any { value.handle !in index.methodsWithStringPrefix(it) }) return "missing required string prefix"
        if (literalValues.any { !value.containsLiteral(it) }) return "missing required literal"
        returnType?.let { if (value.returnType != it) return "return type mismatch" }
        parameterTypes?.let { if (value.parameterTypes != it) return "parameter type mismatch" }
        val params = value.parameterTypes
        if (positionalParams.any { (i, type) -> params.getOrNull(i) != type }) return "positional parameter mismatch"
        if (hasParameters.any { it !in params }) return "missing parameter"
        parameterCount?.let { if (params.size != it) return "parameter count mismatch" }
        if (!flagMask.allSet(info.accessFlags)) return "access flags mismatch"
        inClass?.let { target ->
            val owner = runtime.resolve(target).value.descriptor
            val declares = info.classDescriptor == owner
            if (!declares && !(inheritedMethods && index.inheritedMethods(owner).any { it.handle == value.handle })) {
                return "class mismatch"
            }
        }
        if (calls.any { ref -> !index.methodCalls(value, ref()) }) return "invoke target mismatch"
        if (calledBy.any { caller -> !index.methodCalls(runtime.resolve(caller).value, value) }) return "caller mismatch"
        if (callMatches.any { !index.matchesCall(value, it) }) return "no matching call"
        if (callPredicates.any { predicate -> index.methodRefsOf(value).none(predicate) }) return "no matching call"
        if (opcodes.any { !index.methodHasOpcode(value, it) }) return "opcode mismatch"
        if (opcodeSequences.any { value.indexOfOpcodeSequence(*it.toTypedArray()) == null }) return "opcode sequence mismatch"
        if (predicates.any { !it(value) }) return "custom predicate rejected it"
        return null
    }

    override fun matchReasons(): List<String> = buildList {
        methodName?.let { add("matched name $it") }
        if (stringValues.isNotEmpty()) add("matched strings ${stringValues.joinToString()}")
        if (stringPrefixes.isNotEmpty()) add("matched string prefixes ${stringPrefixes.joinToString()}")
        if (literalValues.isNotEmpty()) add("matched literals ${literalValues.joinToString()}")
        returnType?.let { add("matched return type $it") }
        parameterTypes?.let { add("matched parameter types ${it.joinToString()}") }
        if (positionalParams.isNotEmpty()) add("matched parameters ${positionalParams.entries.joinToString { "${it.key}:${it.value}" }}")
        if (hasParameters.isNotEmpty()) add("matched parameter contains ${hasParameters.joinToString()}")
        parameterCount?.let { add("matched parameter count $it") }
        if (flagMask != 0) add("matched access flags 0x${flagMask.toString(16)}")
        inClass?.let { add("matched class ${it.label}") }
        if (calls.isNotEmpty()) add("matched invoke targets ${calls.joinToString { it().descriptor }}")
        if (calledBy.isNotEmpty()) add("matched caller ${calledBy.joinToString { it.label }}")
        if (callMatches.isNotEmpty()) add("matched ${callMatches.size} indexed call query(s)")
        if (callPredicates.isNotEmpty()) add("matched ${callPredicates.size} call predicate(s)")
        if (opcodes.isNotEmpty()) add("matched opcodes ${opcodes.joinToString()}")
        opcodeSequences.forEach { add("matched opcode sequence ${describeSequence(it)}") }
        if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
        extensionReason()?.let(::add)
    }
}

internal class ClassQuerySpec : QuerySpec<DexClass, ClassRankScope>("class"), ClassQuery {
    val stringValues = mutableListOf<String>()
    val instanceFieldTypes = mutableListOf<String>()
    val superTypes = mutableListOf<String>()
    val interfaceTypes = mutableListOf<String>()
    val sourceFiles = mutableListOf<String>()
    val predicates = mutableListOf<DexClass.() -> Boolean>()

    override fun strings(vararg values: String) { stringValues += values }
    override fun hasInstanceField(type: String) { instanceFieldTypes += descriptor(type) }
    override fun extends(type: String) { superTypes += descriptor(type) }
    override fun implements(type: String) { interfaceTypes += descriptor(type) }
    override fun sourceFile(name: String) { sourceFiles += name }
    override fun custom(predicate: DexClass.() -> Boolean) { predicates += predicate }

    override fun candidates(runtime: PatchRuntime) = runtime.index.classCandidates(this)
    override fun rankScope(index: SearchIndex, value: DexClass) = ClassRankScopeImpl(index, value)
    override fun describe(value: DexClass) = value.descriptor

    override fun mismatch(value: DexClass, runtime: PatchRuntime): String? {
        val index = runtime.index
        if (isHiddenExtension(value.info.dexIndex.toInt(), index)) return "defined by a bundle extension"
        if (stringValues.any { !index.classHasString(value, it) }) return "missing required string"
        if (instanceFieldTypes.any { type -> value.instanceFields.none { it.fieldType == type } }) return "missing instance field"
        if (superTypes.any { !index.classExtends(value, it) }) return "superclass mismatch"
        if (interfaceTypes.any { it !in value.interfaces }) return "interface mismatch"
        if (sourceFiles.any { it != value.sourceFile }) return "source file mismatch"
        if (predicates.any { !it(value) }) return "custom predicate rejected it"
        return null
    }

    override fun matchReasons(): List<String> = buildList {
        if (stringValues.isNotEmpty()) add("matched class strings ${stringValues.joinToString()}")
        if (instanceFieldTypes.isNotEmpty()) add("matched instance fields ${instanceFieldTypes.joinToString()}")
        if (superTypes.isNotEmpty()) add("matched superclass ${superTypes.joinToString()}")
        if (interfaceTypes.isNotEmpty()) add("matched interfaces ${interfaceTypes.joinToString()}")
        if (sourceFiles.isNotEmpty()) add("matched source file ${sourceFiles.joinToString()}")
        if (predicates.isNotEmpty()) add("matched ${predicates.size} custom predicate(s)")
        extensionReason()?.let(::add)
    }
}

/** The engine's instruction pattern: a negative opcode matches any instruction. */
private fun List<Opcode?>.toPattern() = map { it?.value ?: -1 }.toIntArray()

private fun describeSequence(opcodes: List<Opcode?>) = opcodes.joinToString { it?.toString() ?: "any" }

internal class Ranker<S>(val label: String, val block: S.() -> Int)

internal class Scored<T>(val value: T, val score: Int, val reasons: List<String>)

internal class Rejected<T>(val value: T, val failure: String)

private const val MAX_REJECTED = 32

private fun <T> MutableList<Rejected<T>>.addBounded(candidate: Rejected<T>, describe: (Rejected<T>) -> String) {
    add(candidate)
    if (size <= MAX_REJECTED) return
    sortBy(describe)
    subList(MAX_REJECTED, size).clear()
}

internal class Evaluated<T>(
    val accepted: List<Scored<T>>,
    val considered: Int,
    val pipeline: List<String>,
    val exhaustedBy: String?,
    val rejected: List<String>,
    val describe: (T) -> String,
) {
    fun report(debugName: String?, winner: Scored<T>): MatchReport =
        SearchMatchReport(
            debugName ?: "anonymous",
            describe(winner.value),
            considered,
            winner.reasons,
            accepted.filter { it !== winner }.take(3).map { "${describe(it.value)} [score=${it.score}]" },
        )

    fun noMatchReport(debugName: String?): MatchReport =
        SearchMatchReport(
            debugName ?: "anonymous",
            "<no match>",
            considered,
            buildList {
                addAll(pipeline)
                exhaustedBy?.let { add("candidate intersection exhausted at $it") }
                if (rejected.isNotEmpty()) add("no candidate satisfied the full structural query")
            },
            rejected,
        )
}

internal open class RankScopeImpl(protected val index: SearchIndex, override val type: String) : RankScope {
    override fun methods(proto: String): List<Method> =
        index.classFor(type)?.methods?.filter { it.proto == proto }.orEmpty()

    override fun zeroArgListGetters(): Int =
        index.classFor(type)?.methods?.count { it.proto == "()${Type.List}" } ?: 0
}

internal class MethodRankScopeImpl(index: SearchIndex, override val method: Method) :
    RankScopeImpl(index, method.owner), MethodRankScope {
    override val paramCount: Int get() = method.parameterTypes.size
    override fun callSitesFollowedByCast(type: String, lookAhead: Int): Int =
        index.followedByCheckCast(method, descriptor(type), lookAhead)
}

internal class ClassRankScopeImpl(index: SearchIndex, override val classDef: DexClass) :
    RankScopeImpl(index, classDef.descriptor), ClassRankScope

internal class CandidatePool<T>(
    val candidates: List<T>,
    val pipeline: List<String>,
    val considered: Int,
    val nearMissSeed: List<T>,
    val exhaustedBy: String?,
)

internal data class MethodSignature(val owner: String, val name: String, val proto: String)

/** Per-patch caches over the engine's search primitives. */
internal class SearchIndex(private val runtime: PatchRuntime) {
    val allClasses: List<DexClass> by lazy { getAllClasses().map { DexClass(it) } }
    val allMethods: List<Method> by lazy { allMethodHandles().map { Method(it) } }

    private val classByDescriptor = HashMap<String, DexClass?>()
    private val methodBySignature = HashMap<MethodSignature, Method?>()
    private val methodsByClass = HashMap<String, List<Method>>()
    private val inheritedByClass = HashMap<String, List<Method>>()
    private val seeds = HashMap<String, Set<UInt>>()
    private val classesByString = HashMap<String, Set<DexClass>>()
    private val classesByField = HashMap<String, Set<DexClass>>()
    private val classesBySourceFile by lazy { allClasses.groupBy { it.sourceFile } }
    private val refsByMethod = HashMap<UInt, List<MethodRef>>()
    private val invokeSites = HashMap<MethodSignature, List<Pair<UInt, Int>>>()
    private val castCounts = HashMap<Triple<MethodSignature, String, Int>, Int>()
    private val addedDex = HashMap<Int, Boolean>()

    /** True for a DEX the patcher linked in, which is every extension the bundle ships. */
    fun isExtensionDex(dexIndex: Int): Boolean = addedDex.getOrPut(dexIndex) { isAddedDex(dexIndex.toUInt()) }

    fun classFor(descriptor: String): DexClass? =
        classByDescriptor.getOrPut(descriptor) { findClass(descriptor)?.let(::DexClass) }

    fun methodFor(ref: MethodRef): Method? = methodFor(MethodSignature(ref.definingClass, ref.name, ref.proto))

    /**
     * The method a reference names, resolved the way the runtime resolves it:
     * the class's own declaration, else the nearest one it inherits.
     */
    fun methodFor(signature: MethodSignature): Method? =
        methodBySignature.getOrPut(signature) {
            val named = { methods: List<Method> -> methods.firstOrNull { it.name == signature.name && it.proto == signature.proto } }
            named(methodsInClass(signature.owner)) ?: named(inheritedMethods(signature.owner))
        }

    fun methodsInClass(descriptor: String): List<Method> =
        methodsByClass.getOrPut(descriptor) { classFor(descriptor)?.methods.orEmpty() }

    /**
     * Methods this class inherits from the app classes it extends, nearest
     * first, minus the ones it or a nearer ancestor overrides. Constructors and
     * private declarations are not inherited.
     */
    fun inheritedMethods(descriptor: String): List<Method> =
        inheritedByClass.getOrPut(descriptor) {
            val seen = methodsInClass(descriptor).mapTo(mutableSetOf()) { it.name to it.proto }
            classFor(descriptor)?.superclassChain.orEmpty().flatMap { ancestor ->
                ancestor.methods.filter {
                    it.name != "<init>" &&
                        !AccessFlags.PRIVATE.isSet(it.info.accessFlags) &&
                        seen.add(it.name to it.proto)
                }
            }
        }

    private fun seed(key: String, compute: () -> UIntArray): Set<UInt> =
        seeds.getOrPut(key) { compute().toCollection(linkedSetOf()) }

    fun methodsWithName(name: String) = seed("name:$name") { findMethodsByName(name) }
    fun methodsWithReturnType(type: String) = seed("returns:$type") { findMethodsByProto(type, null, null) }
    fun methodsWithExactParameters(types: List<String>) = seed("params:${types.joinToString()}") { findMethodsByProto(null, types, null) }
    fun methodsWithParameter(type: String) = seed("param:$type") { findMethodsByProto(null, null, type) }
    fun methodsWithOpcode(opcode: Opcode) = seed("opcode:$opcode") { findMethodsByOpcodes(intArrayOf(opcode.value)) }
    fun methodsWithOpcodeSequence(opcodes: List<Opcode?>) =
        seed("opcodes:${describeSequence(opcodes)}") { findMethodsByOpcodes(opcodes.toPattern()) }
    fun methodsWithStrings(values: List<String>): Set<UInt> {
        val key = values.distinct().sorted()
        return seed("strings:${key.joinToString("\u0000")}") { findMethodsByStrings(key) }
    }
    fun methodsWithStringPrefix(prefix: String) = seed("stringPrefix:$prefix") {
        findInstructionsByStringContains(prefix)
            .filter { Method(it.method).stringRef(it.index.toInt())?.startsWith(prefix) == true }
            .map { it.method }.toUIntArray()
    }
    fun methodsWithLiteral(value: Long) = seed("literal:$value") { findInstructionsByLiteral(value).map { it.method }.toUIntArray() }

    fun classesWithString(value: String): Set<DexClass> =
        classesByString.getOrPut(value) {
            methodsWithStrings(listOf(value)).asSequence()
                .map { Method(it).owner }
                .distinct()
                .mapNotNull(::classFor)
                .toCollection(linkedSetOf())
        }

    fun classesWithSourceFile(name: String): List<DexClass> = classesBySourceFile[name].orEmpty()

    fun classesWithInstanceFieldType(type: String): Set<DexClass> =
        classesByField.getOrPut(type) {
            findClassesWithInstanceField(type).map(::DexClass).toCollection(linkedSetOf())
        }

    private val matchingCalls = mutableMapOf<MethodRefMatchSpec, Set<UInt>>()

    fun matchesCall(method: Method, spec: MethodRefMatchSpec): Boolean =
        matchingCalls[spec]?.contains(method.handle) ?: methodRefsOf(method).any(spec::matches)

    fun methodsCalling(spec: MethodRefMatchSpec): Set<UInt> = matchingCalls.getOrPut(spec) {
        findCallsMatching(spec.owner, spec.name, spec.returnType, spec.parameters)
            .filter { hit -> !spec.needsPostFilter || Method(hit.method).methodRef(hit.index.toInt())?.let(spec::matches) == true }
            .mapTo(linkedSetOf()) { it.method }
    }

    fun methodHasString(method: Method, value: String): Boolean = method.indexOfFirstString(value) != null

    fun methodHasOpcode(method: Method, opcode: Opcode): Boolean = method.indexOfFirst(opcode) != null

    fun classHasString(classDef: DexClass, value: String): Boolean = classDef.methods.any { methodHasString(it, value) }

    fun classExtends(classDef: DexClass, type: String): Boolean =
        classDef.superclass == type || classDef.superclassChain.any { it.descriptor == type }

    fun methodRefsOf(method: Method): List<MethodRef> =
        refsByMethod.getOrPut(method.handle) { method.instructions.mapNotNull { it.methodRef }.distinct() }

    fun calleesOf(method: Method): List<Method> = methodRefsOf(method).mapNotNull(::methodFor)

    fun methodCalls(method: Method, target: Method): Boolean {
        val signature = signatureOf(target)
        return methodRefsOf(method).any { MethodSignature(it.definingClass, it.name, it.proto) == signature }
    }

    fun methodCalls(method: Method, ref: MethodRef): Boolean =
        invokeSitesFor(MethodSignature(ref.definingClass, ref.name, ref.proto)).any { it.first == method.handle }

    fun followedByCheckCast(target: Method, type: String, lookAhead: Int): Int {
        val signature = signatureOf(target)
        return castCounts.getOrPut(Triple(signature, type, lookAhead)) {
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

    fun methodCandidates(spec: MethodQuerySpec, runtime: PatchRuntime): CandidatePool<Method> {
        val seeds = mutableListOf<Pair<String, Set<UInt>>>()
        spec.inClass?.let { target ->
            val owner = runtime.resolve(target).value.descriptor
            val declared = methodsInClass(owner)
            val inherited = inheritedMethods(owner)
            val label = when {
                spec.inheritedMethods -> "inClass($owner) with ${inherited.size} inherited"
                inherited.isEmpty() -> "inClass($owner)"
                else -> "inClass($owner) declaring only; ${inherited.size} inherited method(s) need inClass(target, inherited = true)"
            }
            val candidates = if (spec.inheritedMethods) declared + inherited else declared
            seeds += label to candidates.map(Method::handle).toSet()
        }
        spec.calls.forEach { resolve ->
            val ref = resolve()
            seeds += "calls(${ref.descriptor})" to invokeSitesFor(MethodSignature(ref.definingClass, ref.name, ref.proto))
                .map { it.first }.toSet()
        }
        spec.calledBy.forEach { target ->
            val method = runtime.resolve(target).value
            seeds += "calledBy(${method.descriptor})" to calleesOf(method).map(Method::handle).toSet()
        }
        if (spec.stringValues.isNotEmpty()) {
            seeds += "strings(${spec.stringValues.joinToString(transform = ::quoted)})" to methodsWithStrings(spec.stringValues)
        }
        spec.stringPrefixes.forEach { seeds += "stringsStartingWith(${quoted(it)})" to methodsWithStringPrefix(it) }
        spec.literalValues.forEach { seeds += "literals($it)" to methodsWithLiteral(it) }
        if (seeds.isEmpty()) {
            spec.callMatches.forEach { seeds += "calls(reference query)" to methodsCalling(it) }
        }
        if (seeds.isEmpty()) {
            spec.methodName?.let { seeds += "name($it)" to methodsWithName(it) }
        }
        if (seeds.isEmpty()) {
            spec.returnType?.let { seeds += "returns($it)" to methodsWithReturnType(it) }
            spec.parameterTypes?.let { seeds += "params(${it.joinToString()})" to methodsWithExactParameters(it) }
            spec.hasParameters.forEach { seeds += "hasParam($it)" to methodsWithParameter(it) }
            spec.opcodes.forEach { seeds += "opcode($it)" to methodsWithOpcode(it) }
            spec.opcodeSequences.forEach { seeds += "opcodeSequence(${describeSequence(it)})" to methodsWithOpcodeSequence(it) }
        }
        if (seeds.isEmpty()) {
            return CandidatePool(allMethods, listOf("no selective constraints; considering all ${allMethods.size} methods"), allMethods.size, allMethods.take(12), null)
        }
        return poolFromSeeds(seeds, "method(s)") { handles -> handles.map(::Method) }
    }

    fun classCandidates(spec: ClassQuerySpec): CandidatePool<DexClass> {
        val seeds = mutableListOf<Pair<String, Set<UInt>>>()
        spec.stringValues.forEach { seeds += "strings(${quoted(it)})" to classesWithString(it).map(DexClass::handle).toSet() }
        spec.sourceFiles.forEach { seeds += "sourceFile(${quoted(it)})" to classesWithSourceFile(it).map(DexClass::handle).toSet() }
        if (seeds.isEmpty()) {
            spec.instanceFieldTypes.forEach { seeds += "hasInstanceField($it)" to classesWithInstanceFieldType(it).map(DexClass::handle).toSet() }
        }
        if (seeds.isEmpty()) {
            return CandidatePool(allClasses, listOf("no selective constraints; considering all ${allClasses.size} classes"), allClasses.size, allClasses.take(12), null)
        }
        return poolFromSeeds(seeds, "class(es)") { handles -> handles.map(::DexClass) }
    }

    private fun <T> poolFromSeeds(seeds: List<Pair<String, Set<UInt>>>, unit: String, wrap: (Collection<UInt>) -> List<T>): CandidatePool<T> {
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
        val considered = when {
            winners.isNotEmpty() -> winners.size
            exhaustedBy != null && lastNonEmpty == null -> 0
            else -> nearMiss.size
        }
        return CandidatePool(wrap(winners), pipeline, considered, wrap(nearMiss), exhaustedBy)
    }

    private fun signatureOf(method: Method) = MethodSignature(method.owner, method.name, method.proto)

    private fun invokeSitesFor(signature: MethodSignature): List<Pair<UInt, Int>> =
        invokeSites.getOrPut(signature) {
            val ref = MethodRef(signature.owner, signature.name, signature.proto)
            findCallsMatching(ref.definingClass, ref.name, ref.returnType, ref.parameterTypes)
                .map { it.method to it.index.toInt() }
        }

    private fun quoted(value: String) = "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
}
