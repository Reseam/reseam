// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.binding.BindingSpec
import app.reseam.patch.binding.ResolvedPath
import app.reseam.patch.binding.emit
import app.reseam.patch.run.Resolution

class BindingTarget
internal constructor(
    debugName: String?,
    private val block: BindingQuery.() -> Unit,
) : Target<ResolvedBinding>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<ResolvedBinding> {
        val binding = BindingSpec().apply(block).compile(runtime, label)
        return Resolution.direct(binding, label, binding.raw.resultType)
    }

    /** The descriptor of the value a path starts from. */
    val sourceType: String
        get() = resolved.raw.resultType

    /** The field the binding was located through, when it came from `fromField`. */
    val sourceField: FieldTarget
        get() = resolved.sourceField ?: error("$label does not originate from a source field")

    /** `raw` applied to `value`: the bound object. */
    fun of(value: ValueRef): ValueRef = resolved.raw.emit(value, label)

    fun member(name: String, on: ValueRef): ValueRef {
        val binding = resolved
        if (name == "raw") return of(on)
        val path =
            binding.members[name]
                ?: error(
                    "$label has no member '$name'; declared: ${binding.members.keys.joinToString()}"
                )
        return path.emit(on, label)
    }
}

fun bind(debugName: String? = null, block: BindingQuery.() -> Unit): BindingTarget =
    BindingTarget(debugName, block)

interface BindingQuery {
    /**
     * The descriptor of the bound object: the source's type inside `raw`, the raw path's result
     * after it.
     */
    val sourceType: String

    fun fromField(debugName: String? = null, block: FieldLocator.() -> Unit)

    fun fromMethod(target: MethodTarget)

    fun fromClass(target: ClassTarget)

    /** The path from the input value to the bound object. */
    fun raw(block: PathQuery.() -> Unit)

    /** A member whose value is the path from the input value, read with [BindingTarget.member]. */
    fun member(name: String, block: PathQuery.() -> Unit)

    /** A member whose value is another binding's root when the path does not say otherwise. */
    fun bind(name: String, target: BindingTarget, block: PathQuery.() -> Unit)
}

interface FieldLocator {
    fun owner(type: String)

    fun firstObjectRead()

    fun firstObjectReadAnyOwner()

    fun nearestObjectReadBeforeString(value: String)

    fun rankBy(label: String, block: RankScope.() -> Int)

    fun requireScoreAtLeast(score: Int)
}

interface PathQuery {
    fun self()

    fun member(name: String)

    fun param(index: Int)

    fun field(type: String)

    fun field(name: String, block: FieldLocator.() -> Unit)

    fun field(target: FieldTarget)

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
}

interface SlotQuery {
    fun firstInstanceOf(type: String)
}

interface MethodRank {
    fun rankBy(label: String, block: MethodRankScope.() -> Int)
}

class ResolvedBinding
internal constructor(
    internal val rootType: String,
    internal val raw: ResolvedPath,
    internal val members: Map<String, ResolvedPath>,
    internal val sourceField: FieldTarget?,
)
