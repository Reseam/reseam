// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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

    /** Indexed field-reference query: reads or writes a matching field. */
    fun accesses(block: FieldRefMatch.() -> Unit)

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

    /**
     * The class is an enum with constants of these names, as `name()` returns them; matches when
     * obfuscation renamed the constants' fields. Seeded from the string index.
     */
    fun enumConstants(vararg names: String)

    /** Has [type] anywhere above it. */
    fun extends(type: String)

    /** Declares [type] itself. */
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
