// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.settings.ToggleSetting
import app.reseam.patch.settings.prependWhen
import app.reseam.patch.settings.returnFalseWhen
import app.reseam.patch.settings.returnNullWhen
import app.reseam.patch.settings.returnTrueWhen
import app.reseam.patch.settings.skipWhen
import kotlin.reflect.KClass

typealias PatchContext = PatchRuntime

interface SearchHandle {
    val debugName: String?
}

interface MethodHandle : SearchHandle {
    val classDescriptor: String
    val methodName: String
    val proto: String
    val method: Method

    fun point(debug: String? = null, block: PointQuery.() -> Unit): PointHandle
    fun constantAfterEnum(enumType: String, enumValue: String): Int
    fun before(block: CodeScope.() -> Unit)
    fun after(block: CodeScope.() -> Unit)
    fun replace(block: CodeScope.() -> Unit)
    fun returnTrueWhen(setting: ToggleSetting)
    fun prependWhen(setting: ToggleSetting, block: CodeScope.() -> Unit)
    fun returnFalseWhen(setting: ToggleSetting)
    fun returnNullWhen(setting: ToggleSetting)
    fun skipWhen(setting: ToggleSetting)
}

interface PointHandle : SearchHandle {
    fun insertBefore(block: CodeScope.() -> Unit)
    fun insertAfter(block: CodeScope.() -> Unit)
}

interface ClassHandle : SearchHandle {
    val descriptor: String
    val classDef: DexClass
}

interface FieldHandle : SearchHandle {
    val owner: String
    val name: String
    val type: String
    val field: FieldRef
}

interface Binding<T : Any> : SearchHandle {
    val runtimeType: KClass<T>
    val sourceType: String

    fun of(raw: ValueRef): ValueRef
    fun member(name: String, on: ValueRef): ValueRef
    fun sourceField(): FieldHandle
}

interface MatchReport {
    val name: String
    val winner: String
    val considered: Int
    val reasons: List<String>
    val nearMisses: List<String>
}

interface MethodQuery {
    fun strings(vararg values: String)
    fun literals(vararg values: Long)
    fun returnType(type: String)
    fun parameterTypes(vararg types: String)
    fun hasParameter(type: String)
    fun inClass(target: ClassHandle)
    fun sameAs(target: MethodHandle)
    fun calls(target: MethodHandle)
    fun calledBy(target: MethodHandle)
    fun hasOpcode(opcode: String)
    fun rankBy(label: String, block: RankScope.() -> Int)
}

interface ClassQuery {
    fun strings(vararg values: String)
    fun hasInstanceField(type: String)
    fun rankBy(label: String, block: RankScope.() -> Int)
}

interface RankScope {
    fun type(): TypeShape
    fun callSites(): CallSiteSet
}

interface TypeShape {
    fun methods(proto: String): List<MethodHandle>
    fun zeroArgListGetters(): Int
}

interface CallSiteSet {
    fun followedByCheckCast(type: String, lookAhead: Int): MatchCount
}

interface MatchCount {
    fun count(): Int
}

interface SlowMethodScan {
    fun hasString(value: String): Boolean
    fun hasOpcode(opcode: String): Boolean
    fun returnType(type: String): Boolean
    fun parameterTypes(vararg types: String): Boolean
}

interface BindingQuery<T : Any> {
    val sourceType: String
    fun fromField(debug: String? = null, block: FieldLocator.() -> Unit)
    fun fromMethod(debug: String? = null, block: MethodQuery.() -> Unit)
    fun fromClass(target: ClassHandle)
    fun raw(block: PathQuery.() -> Unit)
    fun objectValue(name: String, block: PathQuery.() -> Unit)
    fun string(name: String, block: PathQuery.() -> Unit)
    fun context(name: String, block: PathQuery.() -> Unit)
    fun intValue(name: String, block: PathQuery.() -> Unit)
    fun bind(name: String, target: Binding<*>, block: PathQuery.() -> Unit)
    fun list(name: String, element: Binding<*>, block: PathQuery.() -> Unit)
}

interface FieldLocator {
    fun owner(classDescriptor: String)
    fun firstObjectRead()
    fun firstObjectReadAnyOwner()
    fun nearestObjectReadBeforeString(value: String)
    fun rankBy(label: String, block: RankScope.() -> Int)
    fun requireScoreAtLeast(score: Int)
}

interface PathQuery {
    fun self()
    fun member(name: String)
    fun parameter(index: Int)
    fun field(type: String)
    fun field(name: String, block: FieldLocator.() -> Unit)
    fun field(ref: FieldHandle)
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
    fun asBinding(binding: Binding<*>)
}

interface SlotQuery {
    fun firstInstanceOf(type: String)
}

interface MethodRank {
    fun rankBy(label: String, block: RankScope.() -> Int)
}

interface PointQuery {
    fun checkCast(type: String)
    fun previousResult(type: String)
    fun nextBranch(opcode: String, afterInvokeOpcode: String? = null)
}

interface StubBinding {
    fun method(name: String, proto: String, block: CodeScope.() -> Unit)
}

interface CodeScope {
    fun thisObject(): ValueRef
    fun parameter(index: Int): ValueRef
    fun parameterOfType(type: String): ValueRef
    fun parameterLast(): ValueRef
    fun capture(name: String, type: String): ValueRef
    fun enumObject(enumType: String, enumValue: String): ValueRef
    fun int(value: Int): ValueRef
    fun string(value: String): ValueRef
    fun nullObject(): ValueRef
    fun staticCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef
    fun ifTrue(value: ValueRef, block: CodeScope.() -> Unit)
    fun ifEqual(left: ValueRef, right: ValueRef, block: CodeScope.() -> Unit)
    fun ifNotNull(value: ValueRef, block: CodeScope.() -> Unit)
    fun returnVoid()
    fun returnTrue()
    fun returnFalse()
    fun returnObject(value: ValueRef)
    fun returnInt(value: ValueRef)
}

interface ValueRef {
    fun cast(type: String): ValueRef
    fun field(type: String): ValueRef
    fun field(ref: FieldHandle): ValueRef
    fun virtualCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef
    fun interfaceCall(owner: String, name: String, proto: String, vararg args: ValueRef): ValueRef
    fun size(): ValueRef
    fun get(index: ValueRef): ValueRef
    fun minus(value: ValueRef): ValueRef
}

fun PatchRuntime.findMethod(debug: String? = null, block: MethodQuery.() -> Unit): MethodHandle =
    apiState.findMethod(debug, block)

fun PatchRuntime.findMethods(debug: String? = null, block: MethodQuery.() -> Unit): List<MethodHandle> =
    apiState.findMethods(debug, block)

fun PatchRuntime.findClass(debug: String? = null, block: ClassQuery.() -> Unit): ClassHandle =
    apiState.findClass(debug, block)

fun PatchRuntime.classHandle(descriptor: String, debug: String? = null): ClassHandle =
    apiState.classHandle(descriptor, debug)

fun PatchRuntime.wrapField(field: FieldRef, debug: String? = null): FieldHandle =
    apiState.wrapField(field, debug)

fun <T : Any> PatchRuntime.bind(
    runtimeType: KClass<T>,
    debug: String? = null,
    block: BindingQuery<T>.() -> Unit,
): Binding<T> = apiState.bind(runtimeType, debug, block)

inline fun <reified T : Any> PatchRuntime.bind(
    debug: String? = null,
    noinline block: BindingQuery<T>.() -> Unit,
): Binding<T> = bind(T::class, debug, block)

fun PatchRuntime.bindStub(owner: String, block: StubBinding.() -> Unit) {
    apiState.bindStub(owner, block)
}

fun PatchRuntime.scanMethod(debug: String? = null, block: SlowMethodScan.() -> Boolean): MethodHandle =
    apiState.scanMethod(debug, block)

fun PatchRuntime.explain(target: SearchHandle): MatchReport = when (target) {
    is MatchBackedHandle -> target.report
    else -> error("No match report available for ${target::class.simpleName}")
}

internal class PatchApiState(private val ctx: PatchRuntime) {
    private val index: PatchSearchIndex by lazy { PatchSearchIndex(ctx) }
    private val allClasses: List<DexClass>
        get() = index.allClasses
    private val allMethods: List<Method>
        get() = index.allMethods

    private val methodQueryCache = mutableMapOf<MethodQueryCacheKey, EvaluatedMethodQuery>()
    private val classQueryCache = mutableMapOf<ClassQueryCacheKey, EvaluatedClassQuery>()

    fun findMethod(debug: String?, block: MethodQuery.() -> Unit): MethodHandle {
        val spec = MethodQuerySpec().apply(block)
        val report = evaluateMethodQuery(debug, spec)
        val winner = report.winnerMethod
            ?: error(noMatchMessage("method", report.matchReport))
        return MethodHandleImpl(debug, winner, report.matchReport)
    }

    fun findMethods(debug: String?, block: MethodQuery.() -> Unit): List<MethodHandle> {
        val spec = MethodQuerySpec().apply(block)
        return evaluateMethodCandidates(spec).accepted.mapIndexed { index, candidate ->
            MethodHandleImpl(
                debugName = debug,
                method = candidate.method,
                report = SearchMatchReport(
                    name = debug ?: "anonymous",
                    winner = describeMethod(candidate.method),
                    considered = candidate.considered,
                    reasons = candidate.reasons,
                    nearMisses = candidate.nearMisses,
                ),
                ordinal = index,
            )
        }
    }

    fun findClass(debug: String?, block: ClassQuery.() -> Unit): ClassHandle {
        val spec = ClassQuerySpec().apply(block)
        val report = evaluateClassQuery(debug, spec)
        val winner = report.winnerClass
            ?: error(noMatchMessage("class", report.matchReport))
        return ClassHandleImpl(debug, winner, report.matchReport)
    }

    fun <T : Any> bind(
        runtimeType: KClass<T>,
        debug: String?,
        block: BindingQuery<T>.() -> Unit,
    ): Binding<T> = BindingCompiler(this, runtimeType, debug).apply(block).build()

    fun bindStub(owner: String, block: StubBinding.() -> Unit) {
        StubBindingCompiler(this, owner).apply(block)
    }

    fun scanMethod(debug: String?, block: SlowMethodScan.() -> Boolean): MethodHandle {
        val matcher = SlowMethodScanImpl()
        val winner = allMethods.firstOrNull { method ->
            matcher.bind(method)
            block(matcher)
        } ?: error("No method matched '${debug ?: "anonymous"}'")

        return MethodHandleImpl(
            debug,
            winner,
            SearchMatchReport(
                name = debug ?: "anonymous",
                winner = describeMethod(winner),
                considered = allMethods.size,
                reasons = listOf("matched explicit slow scan"),
                nearMisses = emptyList(),
            ),
        )
    }

    internal fun wrapMethod(method: Method, debug: String? = null): MethodHandle =
        MethodHandleImpl(
            debug,
            method,
            SearchMatchReport(
                name = debug ?: describeMethod(method),
                winner = describeMethod(method),
                considered = 1,
                reasons = listOf("wrapped existing method"),
                nearMisses = emptyList(),
            ),
        )

    internal fun wrapClass(classDef: DexClass, debug: String? = null): ClassHandle =
        ClassHandleImpl(
            debug,
            classDef,
            SearchMatchReport(
                name = debug ?: classDef.info.descriptor,
                winner = classDef.info.descriptor,
                considered = 1,
                reasons = listOf("wrapped existing class"),
                nearMisses = emptyList(),
            ),
        )

    internal fun wrapField(field: FieldRef, debug: String? = null): FieldHandle =
        FieldHandleImpl(
            debug,
            field,
            SearchMatchReport(
                name = debug ?: "${field.definingClass}.${field.name}",
                winner = "${field.definingClass}.${field.name}:${field.fieldType}",
                considered = 1,
                reasons = listOf("resolved field"),
                nearMisses = emptyList(),
            ),
        )

    fun classHandle(descriptor: String, debug: String?): ClassHandle =
        wrapClass(index.classFor(descriptor) ?: error("Class not found: $descriptor"), debug)

    private fun evaluateMethodQuery(debug: String?, spec: MethodQuerySpec): MethodQueryResult {
        val evaluated = evaluateMethodCandidates(spec)
        val winner = evaluated.accepted.firstOrNull()
        return MethodQueryResult(
            name = debug ?: "anonymous",
            winnerMethod = winner?.method,
            matchReport = SearchMatchReport(
                name = debug ?: "anonymous",
                winner = winner?.let { describeMethod(it.method) } ?: "<no match>",
                considered = winner?.considered ?: evaluated.considered,
                reasons = winner?.reasons ?: evaluated.noMatchReasons,
                nearMisses = winner?.nearMisses ?: evaluated.noMatchNearMisses,
            ),
        )
    }

    private fun evaluateMethodCandidates(spec: MethodQuerySpec): EvaluatedMethodQuery {
        val key = spec.cacheKey()
        return methodQueryCache.getOrPut(key) {
            evaluateMethodCandidatesUncached(spec)
        }
    }

    private fun evaluateMethodCandidatesUncached(spec: MethodQuerySpec): EvaluatedMethodQuery {
        val pool = index.initialMethodCandidates(spec)
        val scored = mutableListOf<EvaluatedMethodCandidate>()
        val rejected = mutableListOf<RejectedMethodCandidate>()
        val candidatesToCheck = (if (pool.candidates.isNotEmpty()) pool.candidates else pool.nearMissSeed)
            .distinctBy { it.handle }

        for (method in candidatesToCheck) {
            val failureReasons = mutableListOf<String>()
            if (!methodMatches(method, spec, failureReasons)) {
                rejected.addBoundedNearMiss(RejectedMethodCandidate(method, failureReasons))
                continue
            }

            val reasons = pool.pipeline.toMutableList()
            reasons += spec.matchReasons()

            var score = 0
            val rankReasons = mutableListOf<String>()
            for (ranker in spec.rankers) {
                val value = ranker.block(RankScopeImpl(this, method = method))
                score += value
                rankReasons += "${ranker.label}=$value"
            }
            reasons += rankReasons

            scored += EvaluatedMethodCandidate(
                method = method,
                score = score,
                reasons = reasons,
                nearMisses = emptyList(),
                considered = pool.considered,
            )
        }

        val ranked = scored.sortedWith(
            compareByDescending<EvaluatedMethodCandidate> { it.score }
                .thenBy { describeMethod(it.method) }
        )
        val sorted = ranked.mapIndexed { index, candidate ->
            val nearMisses = ranked
                .filterIndexed { otherIndex, _ -> otherIndex != index }
                .take(3)
                .map { other -> "${describeMethod(other.method)} [score=${other.score}]" }
            candidate.copy(nearMisses = nearMisses)
        }

        return EvaluatedMethodQuery(
            accepted = sorted,
            considered = pool.considered,
            noMatchReasons = buildList {
                addAll(pool.pipeline)
                pool.exhaustedBy?.let { add("candidate intersection exhausted at $it") }
                if (rejected.isNotEmpty()) {
                    add("no candidate satisfied the full structural query")
                }
            },
            noMatchNearMisses = rejected
                .sortedWith(
                    compareBy<RejectedMethodCandidate> { it.failures.size }
                        .thenBy { describeMethod(it.method) }
                )
                .take(3)
                .map { rejectedCandidate ->
                    "${describeMethod(rejectedCandidate.method)} [missed: ${rejectedCandidate.failures.joinToString()}]"
                },
        )
    }

    private fun methodMatches(method: Method, spec: MethodQuerySpec, failures: MutableList<String>): Boolean {
        if (spec.stringValues.any { value -> !index.methodHasString(method, value) }) {
            failures += "missing required string"
            return false
        }
        if (spec.literalValues.any { value -> !index.methodHasLiteral(method, value) }) {
            failures += "missing required literal"
            return false
        }
        if (spec.returnType != null && method.returnType != spec.returnType) {
            failures += "return type mismatch"
            return false
        }
        if (spec.parameterTypes != null && method.parameterTypes != spec.parameterTypes) {
            failures += "parameter type mismatch"
            return false
        }
        if (spec.hasParameters.any { it !in method.parameterTypes }) {
            failures += "missing parameter"
            return false
        }
        if (spec.inClass != null && method.info.classDescriptor != spec.inClass!!.descriptor) {
            failures += "class mismatch"
            return false
        }
        if (spec.sameAs != null && !sameMethod(method, spec.sameAs!!.method)) {
            failures += "method mismatch"
            return false
        }
        if (spec.calls.any { target -> !methodCalls(method, target.method) }) {
            failures += "invoke target mismatch"
            return false
        }
        if (spec.calledBy.any { caller -> !methodCalls(caller.method, method) }) {
            failures += "caller mismatch"
            return false
        }
        if (spec.opcodes.any { opcodeName -> !methodHasOpcode(method, opcodeName) }) {
            failures += "opcode mismatch"
            return false
        }
        return true
    }

    private fun evaluateClassQuery(debug: String?, spec: ClassQuerySpec): ClassQueryResult {
        val evaluated = evaluateClassCandidates(spec)
        val winner = evaluated.accepted.firstOrNull()?.classDef
        return ClassQueryResult(
            name = debug ?: "anonymous",
            winnerClass = winner,
            matchReport = SearchMatchReport(
                name = debug ?: "anonymous",
                winner = winner?.info?.descriptor ?: "<no match>",
                considered = winner?.let { evaluated.accepted.first().considered } ?: evaluated.considered,
                reasons = evaluated.accepted.firstOrNull()?.reasons ?: evaluated.noMatchReasons,
                nearMisses = evaluated.accepted.firstOrNull()?.nearMisses ?: evaluated.noMatchNearMisses,
            ),
        )
    }

    private fun evaluateClassCandidates(spec: ClassQuerySpec): EvaluatedClassQuery {
        val key = spec.cacheKey()
        return classQueryCache.getOrPut(key) {
            evaluateClassCandidatesUncached(spec)
        }
    }

    private fun evaluateClassCandidatesUncached(spec: ClassQuerySpec): EvaluatedClassQuery {
        val pool = index.initialClassCandidates(spec)
        val scored = mutableListOf<EvaluatedClassCandidate>()
        val rejected = mutableListOf<RejectedClassCandidate>()
        val candidatesToCheck = (if (pool.candidates.isNotEmpty()) pool.candidates else pool.nearMissSeed)
            .distinctBy { it.info.descriptor }

        for (classDef in candidatesToCheck) {
            val failures = mutableListOf<String>()
            if (!classMatches(classDef, spec, failures)) {
                rejected.addBoundedNearMiss(RejectedClassCandidate(classDef, failures))
                continue
            }

            val reasons = pool.pipeline.toMutableList()
            reasons += spec.matchReasons()

            var score = 0
            val rankReasons = mutableListOf<String>()
            for (ranker in spec.rankers) {
                val value = ranker.block(RankScopeImpl(this, classDef = classDef))
                score += value
                rankReasons += "${ranker.label}=$value"
            }
            reasons += rankReasons

            scored += EvaluatedClassCandidate(classDef, score, reasons, emptyList(), pool.considered)
        }

        val ranked = scored.sortedWith(
            compareByDescending<EvaluatedClassCandidate> { it.score }
                .thenBy { it.classDef.info.descriptor }
        )
        val sorted = ranked.mapIndexed { index, candidate ->
            val nearMisses = rankedNearMissesForClass(ranked, index)
            candidate.copy(nearMisses = nearMisses)
        }

        return EvaluatedClassQuery(
            accepted = sorted,
            considered = pool.considered,
            noMatchReasons = buildList {
                addAll(pool.pipeline)
                pool.exhaustedBy?.let { add("candidate intersection exhausted at $it") }
                if (rejected.isNotEmpty()) {
                    add("no candidate satisfied the full structural query")
                }
            },
            noMatchNearMisses = rejected
                .sortedWith(
                    compareBy<RejectedClassCandidate> { it.failures.size }
                        .thenBy { it.classDef.info.descriptor }
                )
                .take(3)
                .map { candidate ->
                    "${candidate.classDef.info.descriptor} [missed: ${candidate.failures.joinToString()}]"
                },
        )
    }

    private fun classMatches(classDef: DexClass, spec: ClassQuerySpec, failures: MutableList<String>): Boolean {
        if (spec.stringValues.any { value -> !index.classHasString(classDef, value) }) {
            failures += "missing required string"
            return false
        }
        if (spec.instanceFieldTypes.any { fieldType -> !index.classHasInstanceFieldType(classDef, fieldType) }) {
            failures += "missing instance field"
            return false
        }
        return true
    }

    internal fun callersOf(target: Method): List<Method> {
        return index.callersOf(target)
    }

    internal fun methodHasOpcode(method: Method, opcodeName: String): Boolean {
        val opcode = OPCODE_NAMES[opcodeName]
            ?: error("Unknown opcode name '$opcodeName'")
        return index.methodHasOpcode(method, opcode)
    }

    internal fun sameMethod(left: Method, right: Method): Boolean =
        left.info.classDescriptor == right.info.classDescriptor &&
            left.info.methodName == right.info.methodName &&
            left.info.proto == right.info.proto

    internal fun sameMethodRef(ref: MethodRef, method: Method): Boolean =
        ref.definingClass == method.info.classDescriptor &&
            ref.name == method.info.methodName &&
            ref.proto == method.info.proto

    internal fun methodCalls(method: Method, target: Method): Boolean =
        index.calleesOf(method).any { callee -> sameMethod(callee, target) }

    internal fun classFor(descriptor: String): DexClass? =
        index.classFor(descriptor)

    internal fun followedByCheckCast(method: Method, type: String, lookAhead: Int): Int =
        index.followedByCheckCast(method, type, lookAhead)

    internal fun describeMethod(method: Method): String =
        "${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"

    internal fun context(): PatchRuntime = ctx

    private fun noMatchMessage(kind: String, report: MatchReport): String =
        buildString {
            append("No ")
            append(kind)
            append(" matched '")
            append(report.name)
            append("'. Searched ")
            append(report.considered)
            append(if (kind == "method") " method candidate(s)." else " class candidate(s).")
            if (report.reasons.isNotEmpty()) {
                append("\nReasons: ")
                append(report.reasons.joinToString("; "))
            }
            if (report.nearMisses.isNotEmpty()) {
                append("\nNear misses: ")
                append(report.nearMisses.joinToString("; "))
            }
        }

    private fun rankedNearMissesForClass(
        ranked: List<EvaluatedClassCandidate>,
        currentIndex: Int,
    ): List<String> =
        ranked
            .filterIndexed { index, _ -> index != currentIndex }
            .take(3)
            .map { "${it.classDef.info.descriptor} [score=${it.score}]" }
}

private interface CacheableQueryKey

internal data class MethodQueryCacheKey(
    val stringValues: List<String>,
    val literalValues: List<Long>,
    val returnType: String?,
    val parameterTypes: List<String>?,
    val hasParameters: List<String>,
    val inClass: String?,
    val sameAs: MethodSignature?,
    val calls: List<MethodSignature>,
    val calledBy: List<MethodSignature>,
    val opcodes: List<String>,
    val rankers: List<Any>,
) : CacheableQueryKey

internal data class ClassQueryCacheKey(
    val stringValues: List<String>,
    val instanceFieldTypes: List<String>,
    val rankers: List<Any>,
) : CacheableQueryKey

internal data class RankerSpec(
    val label: String,
    val block: RankScope.() -> Int,
)

private data class RejectedMethodCandidate(
    val method: Method,
    val failures: List<String>,
)

private data class RejectedClassCandidate(
    val classDef: DexClass,
    val failures: List<String>,
)

private const val MAX_REJECTED_NEAR_MISSES = 32

private fun MutableList<RejectedMethodCandidate>.addBoundedNearMiss(candidate: RejectedMethodCandidate) {
    add(candidate)
    if (size <= MAX_REJECTED_NEAR_MISSES) return
    sortWith(
        compareBy<RejectedMethodCandidate> { it.failures.size }
            .thenBy { rejectedMethodName(it.method) }
    )
    subList(MAX_REJECTED_NEAR_MISSES, size).clear()
}

private fun MutableList<RejectedClassCandidate>.addBoundedNearMiss(candidate: RejectedClassCandidate) {
    add(candidate)
    if (size <= MAX_REJECTED_NEAR_MISSES) return
    sortWith(
        compareBy<RejectedClassCandidate> { it.failures.size }
            .thenBy { it.classDef.info.descriptor }
    )
    subList(MAX_REJECTED_NEAR_MISSES, size).clear()
}

private fun rejectedMethodName(method: Method): String =
    "${method.info.classDescriptor}->${method.info.methodName}${method.info.proto}"

private data class EvaluatedMethodQuery(
    val accepted: List<EvaluatedMethodCandidate>,
    val considered: Int,
    val noMatchReasons: List<String>,
    val noMatchNearMisses: List<String>,
)

private data class EvaluatedClassQuery(
    val accepted: List<EvaluatedClassCandidate>,
    val considered: Int,
    val noMatchReasons: List<String>,
    val noMatchNearMisses: List<String>,
)

private interface MatchBackedHandle : SearchHandle {
    val report: MatchReport
}

private data class SearchMatchReport(
    override val name: String,
    override val winner: String,
    override val considered: Int,
    override val reasons: List<String>,
    override val nearMisses: List<String>,
) : MatchReport

internal class MethodHandleImpl(
    override val debugName: String?,
    override val method: Method,
    override val report: MatchReport,
    private val ordinal: Int = 0,
) : MethodHandle, MatchBackedHandle {
    override val classDescriptor: String
        get() = method.info.classDescriptor
    override val methodName: String
        get() = method.info.methodName
    override val proto: String
        get() = method.info.proto

    override fun point(debug: String?, block: PointQuery.() -> Unit): PointHandle =
        MethodPointCompiler(this, debug).apply(block).build()

    override fun constantAfterEnum(enumType: String, enumValue: String): Int =
        method.constantAfterEnum(enumType, enumValue)

    override fun before(block: CodeScope.() -> Unit) {
        method.insertCodeBefore(block)
    }

    override fun after(block: CodeScope.() -> Unit) {
        method.insertCodeBeforeReturns(block)
    }

    override fun replace(block: CodeScope.() -> Unit) {
        method.replaceWithCode(block)
    }

    override fun returnTrueWhen(setting: ToggleSetting) {
        method.returnTrueWhen(setting)
    }

    override fun prependWhen(setting: ToggleSetting, block: CodeScope.() -> Unit) {
        method.prependWhen(setting, block)
    }

    override fun returnFalseWhen(setting: ToggleSetting) {
        method.returnFalseWhen(setting)
    }

    override fun returnNullWhen(setting: ToggleSetting) {
        method.returnNullWhen(setting)
    }

    override fun skipWhen(setting: ToggleSetting) {
        method.skipWhen(setting)
    }

    override fun equals(other: Any?): Boolean =
        other is MethodHandleImpl &&
            classDescriptor == other.classDescriptor &&
            methodName == other.methodName &&
            proto == other.proto &&
            ordinal == other.ordinal

    override fun hashCode(): Int =
        (((classDescriptor.hashCode() * 31 + methodName.hashCode()) * 31 + proto.hashCode()) * 31) + ordinal
}

private class ClassHandleImpl(
    override val debugName: String?,
    override val classDef: DexClass,
    override val report: MatchReport,
) : ClassHandle, MatchBackedHandle {
    override val descriptor: String
        get() = classDef.info.descriptor
}

private class FieldHandleImpl(
    override val debugName: String?,
    private val fieldRef: FieldRef,
    override val report: MatchReport,
) : FieldHandle, MatchBackedHandle {
    override val field: FieldRef
        get() = fieldRef
    override val owner: String
        get() = fieldRef.definingClass
    override val name: String
        get() = fieldRef.name
    override val type: String
        get() = fieldRef.fieldType
}

private data class MethodQueryResult(
    val name: String,
    val winnerMethod: Method?,
    val matchReport: MatchReport,
)

private data class ClassQueryResult(
    val name: String,
    val winnerClass: DexClass?,
    val matchReport: MatchReport,
)

private data class EvaluatedMethodCandidate(
    val method: Method,
    val score: Int,
    val reasons: List<String>,
    val nearMisses: List<String>,
    val considered: Int,
)

private data class EvaluatedClassCandidate(
    val classDef: DexClass,
    val score: Int,
    val reasons: List<String>,
    val nearMisses: List<String>,
    val considered: Int,
)

internal class MethodQuerySpec : MethodQuery {
    val stringValues = mutableListOf<String>()
    val literalValues = mutableListOf<Long>()
    var returnType: String? = null
    var parameterTypes: List<String>? = null
    val hasParameters = mutableListOf<String>()
    var inClass: ClassHandle? = null
    var sameAs: MethodHandle? = null
    val calls = mutableListOf<MethodHandle>()
    val calledBy = mutableListOf<MethodHandle>()
    val opcodes = mutableListOf<String>()
    val rankers = mutableListOf<RankerSpec>()

    override fun strings(vararg values: String) {
        stringValues += values
    }

    override fun literals(vararg values: Long) {
        literalValues += values.toList()
    }

    override fun returnType(type: String) {
        returnType = type
    }

    override fun parameterTypes(vararg types: String) {
        parameterTypes = types.toList()
    }

    override fun hasParameter(type: String) {
        hasParameters += type
    }

    override fun inClass(target: ClassHandle) {
        inClass = target
    }

    override fun sameAs(target: MethodHandle) {
        sameAs = target
    }

    override fun calls(target: MethodHandle) {
        calls += target
    }

    override fun calledBy(target: MethodHandle) {
        calledBy += target
    }

    override fun hasOpcode(opcode: String) {
        opcodes += opcode
    }

    override fun rankBy(label: String, block: RankScope.() -> Int) {
        rankers += RankerSpec(label, block)
    }

    fun cacheKey(): MethodQueryCacheKey =
        MethodQueryCacheKey(
            stringValues = stringValues.toList(),
            literalValues = literalValues.toList(),
            returnType = returnType,
            parameterTypes = parameterTypes?.toList(),
            hasParameters = hasParameters.toList(),
            inClass = inClass?.descriptor,
            sameAs = sameAs?.let { MethodSignature(it.classDescriptor, it.methodName, it.proto) },
            calls = calls.map { MethodSignature(it.classDescriptor, it.methodName, it.proto) },
            calledBy = calledBy.map { MethodSignature(it.classDescriptor, it.methodName, it.proto) },
            opcodes = opcodes.toList(),
            rankers = rankers.map { it.block },
        )

    fun matchReasons(): List<String> = buildList {
        if (stringValues.isNotEmpty()) add("matched strings ${stringValues.joinToString()}")
        if (literalValues.isNotEmpty()) add("matched literals ${literalValues.joinToString()}")
        returnType?.let { add("matched return type $it") }
        parameterTypes?.let { add("matched parameter types ${it.joinToString()}") }
        if (hasParameters.isNotEmpty()) add("matched parameter contains ${hasParameters.joinToString()}")
        inClass?.let { add("matched class ${it.descriptor}") }
        if (calls.isNotEmpty()) add("matched invoke target ${calls.joinToString { handle -> "${handle.classDescriptor}->${handle.methodName}${handle.proto}" }}")
        if (calledBy.isNotEmpty()) add("matched caller ${calledBy.joinToString { handle -> "${handle.classDescriptor}->${handle.methodName}${handle.proto}" }}")
        if (opcodes.isNotEmpty()) add("matched opcodes ${opcodes.joinToString()}")
    }
}

internal class ClassQuerySpec : ClassQuery {
    val stringValues = mutableListOf<String>()
    val instanceFieldTypes = mutableListOf<String>()
    val rankers = mutableListOf<RankerSpec>()

    override fun strings(vararg values: String) {
        stringValues += values
    }

    override fun hasInstanceField(type: String) {
        instanceFieldTypes += type
    }

    override fun rankBy(label: String, block: RankScope.() -> Int) {
        rankers += RankerSpec(label, block)
    }

    fun cacheKey(): ClassQueryCacheKey =
        ClassQueryCacheKey(
            stringValues = stringValues.toList(),
            instanceFieldTypes = instanceFieldTypes.toList(),
            rankers = rankers.map { it.block },
        )

    fun matchReasons(): List<String> = buildList {
        if (stringValues.isNotEmpty()) add("matched class strings ${stringValues.joinToString()}")
        if (instanceFieldTypes.isNotEmpty()) add("matched instance fields ${instanceFieldTypes.joinToString()}")
    }
}

internal class RankScopeImpl(
    private val state: PatchApiState,
    private val method: Method? = null,
    private val classDef: DexClass? = null,
    private val typeDescriptor: String? = null,
) : RankScope {
    override fun type(): TypeShape {
        val descriptor = typeDescriptor
            ?: classDef?.info?.descriptor
            ?: method?.info?.classDescriptor
            ?: error("No candidate type available")
        return TypeShapeImpl(state, descriptor)
    }

    override fun callSites(): CallSiteSet =
        CallSiteSetImpl(state, method ?: error("callSites() requires a method candidate"))
}

private class TypeShapeImpl(
    private val state: PatchApiState,
    private val descriptor: String,
) : TypeShape {
    override fun methods(proto: String): List<MethodHandle> =
        state.classFor(descriptor)
            ?.methods
            ?.filter { it.info.proto == proto }
            ?.map { state.wrapMethod(it, debug = "${descriptor}#$proto") }
            ?: emptyList()

    override fun zeroArgListGetters(): Int =
        state.classFor(descriptor)
            ?.methods
            ?.count { it.info.proto == "()Ljava/util/List;" }
            ?: 0
}

private class CallSiteSetImpl(
    private val state: PatchApiState,
    private val method: Method,
) : CallSiteSet {
    override fun followedByCheckCast(type: String, lookAhead: Int): MatchCount {
        return MatchCountImpl(state.followedByCheckCast(method, type, lookAhead))
    }
}

private class MatchCountImpl(private val value: Int) : MatchCount {
    override fun count(): Int = value
}

private class SlowMethodScanImpl : SlowMethodScan {
    private lateinit var method: Method

    fun bind(method: Method) {
        this.method = method
    }

    override fun hasString(value: String): Boolean =
        method.indexOfFirstString(value) != null

    override fun hasOpcode(opcode: String): Boolean {
        val value = OPCODE_NAMES[opcode]
            ?: error("Unknown opcode '$opcode'")
        return method.instructions.any { it.opcode() == value }
    }

    override fun returnType(type: String): Boolean =
        method.returnType == type

    override fun parameterTypes(vararg types: String): Boolean =
        method.parameterTypes == types.toList()
}

internal val OPCODE_NAMES: Map<String, Int> by lazy {
    Opcodes::class.java.fields
        .filter { it.type == Int::class.javaPrimitiveType }
        .associate { it.name to it.getInt(null) }
}
