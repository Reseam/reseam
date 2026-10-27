// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.findInstructionsByString
import app.reseam.patch.native.findInstructionsByStringContains
import app.reseam.patch.native.findInstructionsByStringPrefix
import app.reseam.patch.native.instructionStringRef
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.InstructionHit
import app.reseam.patch.types.MethodRef

class BytecodeScope internal constructor() {
    val classes: List<DexClass>
        get() = ActiveRuntime.current.index.allClasses

    fun findClass(name: String): DexClass? = ActiveRuntime.current.index.classFor(descriptor(name))

    /** Every class with [type] anywhere above it. */
    fun classesExtending(type: String): List<DexClass> =
        ActiveRuntime.current.index.classesExtending(descriptor(type)).toList()

    /** Every class declaring [type] itself. */
    fun classesImplementing(type: String): List<DexClass> =
        ActiveRuntime.current.index.classesImplementing(descriptor(type)).toList()

    /**
     * Rewrites every `const-string` equal to `old` in the app; returns how many constants changed.
     */
    fun replaceAllStrings(old: String, new: String): Int =
        findInstructionsByString(old)
            .map { Method(it.method) }
            .distinctBy { it.handle }
            .sumOf { it.replaceAllStrings(old, new) }

    /**
     * Rewrites every `const-string` containing [substring] through [transform], which returns the
     * replacement or null to leave that constant as it is. Returns how many constants changed.
     *
     * Seeded from the string index, so only methods holding a matching constant are visited. Use it
     * when a value is embedded in larger constants rather than stored on its own, such as an
     * authority inside a `content://` URI; [replaceAllStrings] covers the whole-string case.
     */
    fun replaceStringsContaining(substring: String, transform: (String) -> String?): Int =
        replaceLoaded(findInstructionsByStringContains(substring), transform)

    /** [replaceStringsContaining] for constants starting with [prefix]. */
    fun replaceStringsStartingWith(prefix: String, transform: (String) -> String?): Int =
        replaceLoaded(findInstructionsByStringPrefix(prefix), transform)

    private fun replaceLoaded(hits: List<InstructionHit>, transform: (String) -> String?): Int =
        hits
            .mapNotNull { instructionStringRef(it.method, it.index) }
            .distinct()
            .sumOf { old ->
                transform(old)?.takeIf { it != old }?.let { replaceAllStrings(old, it) } ?: 0
            }

    /**
     * Every call to `from` in the app becomes a call to the static `to`, receiver first for
     * instance methods. Calls through `super`, constructor calls, and calls from extensions are
     * left alone, so `to` can call `from`. Returns how many call sites changed.
     */
    fun redirectCalls(from: MethodRef, to: ExtMethod): Int {
        val callers =
            app.reseam.patch.methods("callers of ${from.descriptor}") {
                calls(from)
            }
        return callers
            .points("redirect ${from.descriptor}") {
                invoke(*redirectableInvokes.toTypedArray()) {
                    owner(from.definingClass)
                    name(from.name)
                    params(*from.parameterTypes.toTypedArray())
                    returns(from.returnType)
                }
            }
            .all
            .redirectTo(to)
    }

    /**
     * [redirectCalls] with `from` derived from `to`: `to` takes the same parameters as
     * `owner.name`, preceded by the receiver when `owner.name` is an instance method, and returns
     * the same type.
     */
    fun redirectCalls(owner: String, name: String, to: ExtMethod): Int {
        val ownerDesc = descriptor(owner)
        val params = to.ref.parameterTypes
        val fromParams = if (params.firstOrNull() == ownerDesc) params.drop(1) else params
        return redirectCalls(
            MethodRef(ownerDesc, name, proto(to.ref.returnType, *fromParams.toTypedArray())),
            to,
        )
    }
}
