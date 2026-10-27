// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.run.Resolution
import app.reseam.patch.search.ClassQuerySpec

class ClassTarget
internal constructor(
    debugName: String?,
    private val resolver: (PatchRuntime) -> Resolution<DexClass>,
) : Target<DexClass>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<DexClass> = resolver(runtime)

    val classDef: DexClass
        get() = resolved

    val descriptor: String
        get() = classDef.descriptor

    internal companion object {
        fun of(classDef: DexClass, debugName: String? = null): ClassTarget =
            ClassTarget(debugName) {
                Resolution.direct(classDef, debugName ?: classDef.descriptor, classDef.descriptor)
            }
    }
}

fun classTarget(debugName: String, resolve: PatchRuntime.() -> DexClass): ClassTarget =
    ClassTarget(debugName) { runtime ->
        runtime.resolve().let { Resolution.direct(it, debugName, it.descriptor) }
    }

/** A class by name, dotted or as a descriptor. */
fun klass(name: String): ClassTarget {
    val desc = descriptor(name)
    return ClassTarget(className(desc)) { runtime ->
        val found = runtime.index.requireClass(desc)
        Resolution.direct(found, className(desc), desc)
    }
}

/** The one class satisfying the query. */
fun klass(debugName: String, block: ClassQuery.() -> Unit): ClassTarget =
    ClassTarget(debugName) { runtime ->
        ClassQuerySpec().apply(block).resolveOne(runtime, debugName)
    }
