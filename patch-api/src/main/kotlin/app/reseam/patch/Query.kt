// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.types.MethodRef

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

    /**
     * Every access flag the method must carry: `flags(AccessFlags.PUBLIC or AccessFlags.STATIC)`
     * needs both.
     */
    fun flags(mask: Int)

    /**
     * The method belongs to this class. `inherited` widens that to the methods the class inherits
     * from app classes it extends, which is what a call site on the class resolves to; by default
     * only its own declarations match.
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
     * The method contains these opcodes back to back, `null` matching any one instruction. Ordered
     * instruction shape is a last-resort constraint: it seeds only when nothing more selective is
     * in the query.
     */
    fun opcodeSequence(vararg opcodes: Opcode?)

    /**
     * The candidate satisfies `predicate`, for what the rest of the query cannot say. A filter and
     * never a seed, so it needs a selective constraint beside it or the query considers every
     * method in the app.
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
     * The candidate satisfies `predicate`, for what the rest of the query cannot say. A filter and
     * never a seed, so it needs a selective constraint beside it or the query considers every class
     * in the app.
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

    /**
     * How many call sites of the candidate cast the result to `type` within `lookAhead`
     * instructions.
     */
    fun callSitesFollowedByCast(type: String, lookAhead: Int = 40): Int
}

interface ClassRankScope : RankScope {
    val classDef: DexClass
}

internal enum class QueryMultiplicity {
    UNIQUE,
    FIRST,
}

internal enum class SearchDomain {
    APP,
    INCLUDE_EXTENSIONS,
}

internal enum class MethodScope {
    DECLARED,
    INHERITED,
}

internal abstract class QuerySpec<T : Any, S : RankScope>(private val kind: String) {
    private val rankers = mutableListOf<Ranker<S>>()
    private var multiplicity = QueryMultiplicity.UNIQUE
    private var domain = SearchDomain.APP

    fun rankBy(label: String, block: S.() -> Int) {
        rankers += Ranker(label, block)
    }

    fun first() {
        multiplicity = QueryMultiplicity.FIRST
    }

    fun includeExtensions() {
        domain = SearchDomain.INCLUDE_EXTENSIONS
    }

    /** Excludes extensions by default so lazy linking cannot change query results. */
    protected fun isHiddenExtension(dexIndex: Int, index: SearchIndex): Boolean =
        domain == SearchDomain.APP && index.isExtensionDex(dexIndex)

    protected fun extensionReason(): String? =
        if (domain == SearchDomain.INCLUDE_EXTENSIONS) "including extension classes" else null

    protected abstract fun candidates(runtime: PatchRuntime): CandidatePool<T>

    protected abstract fun mismatch(value: T, runtime: PatchRuntime): String?

    protected abstract fun matchReasons(): List<String>

    protected abstract fun rankScope(index: SearchIndex, value: T): S

    protected abstract fun describe(value: T): String

    fun resolveOne(runtime: PatchRuntime, debugName: String?): Resolution<T> {
        val evaluated = evaluate(runtime)
        val winner =
            evaluated.accepted.firstOrNull()
                ?: error(noMatchMessage(kind, evaluated.noMatchReport(debugName)))
        val runnerUp = evaluated.accepted.getOrNull(1)
        check(
            multiplicity == QueryMultiplicity.FIRST ||
                runnerUp == null ||
                rankers.isNotEmpty() && winner.score > runnerUp.score
        ) {
            "${evaluated.accepted.size} ${kind}s matched '${debugName ?: "anonymous"}'; add constraints, rank them, or take first(): " +
                evaluated.accepted.take(6).joinToString("; ") {
                    "${describe(it.value)} [score=${it.score}]"
                }
        }
        return Resolution(winner.value, evaluated.report(debugName, winner))
    }

    fun resolveAll(runtime: PatchRuntime, debugName: String?): Resolution<List<T>> {
        val evaluated = evaluate(runtime)
        val report =
            SearchMatchReport(
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
            scored +=
                Scored(
                    value,
                    scores.sumOf { it.second },
                    pool.pipeline + matchReasons() + scores.map { "${it.first}=${it.second}" },
                )
        }
        return Evaluated(
            accepted =
                scored.sortedWith(
                    compareByDescending<Scored<T>> { it.score }.thenBy { describe(it.value) }
                ),
            considered = pool.considered,
            pipeline = pool.pipeline,
            exhaustedBy = pool.exhaustedBy,
            rejected =
                rejected
                    .sortedBy { describe(it.value) }
                    .take(3)
                    .map { "${describe(it.value)} [missed: ${it.failure}]" },
            describe = ::describe,
        )
    }
}

internal fun List<Opcode?>.toPattern() = map { it?.value ?: -1 }.toIntArray()

internal fun describeSequence(opcodes: List<Opcode?>) = opcodes.joinToString {
    it?.toString() ?: "any"
}

internal fun quoted(value: String) = "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
