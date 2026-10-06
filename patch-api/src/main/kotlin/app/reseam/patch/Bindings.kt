// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

class BindingTarget
internal constructor(
    debugName: String?,
    private val block: BindingQuery.() -> Unit,
) : Target<ResolvedBinding>(debugName) {
    override fun resolve(runtime: PatchRuntime): Resolution<ResolvedBinding> {
        val binding = BindingSpec().apply(block).compile(runtime, label)
        return Resolution(binding, wrapped(label, binding.raw.resultType))
    }

    /** The descriptor of the value a path starts from. */
    val sourceType: String
        get() = resolved.raw.resultType

    /** The field the binding was located through, when it came from `fromField`. */
    val sourceField: FieldTarget
        get() = resolved.sourceField ?: error("$label does not originate from a source field")

    /** `raw` applied to `value`: the bound object. */
    fun of(value: ValueRef): ValueRef = resolved.apply(value, resolved.raw, label)

    fun member(name: String, on: ValueRef): ValueRef {
        val binding = resolved
        if (name == "raw") return of(on)
        val path =
            binding.members[name]
                ?: error(
                    "$label has no member '$name'; declared: ${binding.members.keys.joinToString()}"
                )
        return binding.apply(on, path, label)
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

    fun objectValue(name: String, block: PathQuery.() -> Unit)

    fun string(name: String, block: PathQuery.() -> Unit)

    fun context(name: String, block: PathQuery.() -> Unit)

    fun intValue(name: String, block: PathQuery.() -> Unit)

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
}

interface SlotQuery {
    fun firstInstanceOf(type: String)
}

interface MethodRank {
    fun rankBy(label: String, block: MethodRankScope.() -> Int)
}

internal sealed interface PathStep {
    class FieldRead(val field: FieldRef) : PathStep

    class VirtualCall(val ref: MethodRef) : PathStep

    class InterfaceCall(val ref: MethodRef) : PathStep

    class CastValue(val type: String) : PathStep

    class ObjectSlot(val fields: List<FieldRef>, val targetType: String) : PathStep

    data object ListFirst : PathStep

    data object ListLast : PathStep
}

internal class ResolvedPath(val rootType: String, val resultType: String, val steps: List<PathStep>)

class ResolvedBinding
internal constructor(
    internal val rootType: String,
    internal val raw: ResolvedPath,
    internal val members: Map<String, ResolvedPath>,
    internal val sourceField: FieldTarget?,
) {
    internal fun apply(base: ValueRef, path: ResolvedPath, label: String): ValueRef {
        val emitter = base.emitter()
        val start = with(emitter) { base.impl() }
        var current = adaptInput(emitter, start, path.rootType, label)
        if (path.steps.isEmpty()) return current

        val nullLabel = emitter.nextLabel()
        val doneLabel = emitter.nextLabel()
        val result = emitter.allocTemp(registerWordCount(path.resultType))

        for (step in path.steps) {
            current =
                when (step) {
                    is PathStep.FieldRead ->
                        emitter
                            .readField(nullChecked(emitter, current, nullLabel), step.field)
                            .value(emitter)
                    is PathStep.VirtualCall ->
                        emitter
                            .invoke(
                                Opcode.INVOKE_VIRTUAL,
                                step.ref,
                                listOf(nullChecked(emitter, current, nullLabel)),
                            )
                            .value(emitter)
                    is PathStep.InterfaceCall ->
                        emitter
                            .invoke(
                                Opcode.INVOKE_INTERFACE,
                                step.ref,
                                listOf(nullChecked(emitter, current, nullLabel)),
                            )
                            .value(emitter)
                    PathStep.ListFirst -> {
                        emitter.ifZero(current, nullLabel)
                        val empty =
                            emitter
                                .invoke(
                                    Opcode.INVOKE_INTERFACE,
                                    MethodRef(Type.List, "isEmpty", "()Z"),
                                    listOf(current),
                                )
                                .value(emitter)
                        emitter.ifNonZero(empty, nullLabel)
                        current.get(emitter.int(0)).value(emitter)
                    }
                    PathStep.ListLast -> {
                        emitter.ifZero(current, nullLabel)
                        current.get(current.size() - emitter.int(1)).value(emitter)
                    }
                    is PathStep.CastValue -> emitter.cast(current, step.type).value(emitter)
                    is PathStep.ObjectSlot -> {
                        val obj = nullChecked(emitter, current, nullLabel)
                        val slotDone = emitter.nextLabel()
                        val slot = emitter.allocTemp(constraint = RegisterConstraint.LOW)
                        emitter.constZero(slot, step.targetType)
                        val slotValue = emitter.Value(slot, step.targetType)
                        for (field in step.fields) {
                            val next = emitter.nextLabel()
                            val probe = emitter.readField(obj, field).value(emitter)
                            val isInstance = emitter.instanceOf(probe, step.targetType)
                            emitter.ifZero(isInstance, next)
                            emitter.moveValue(slot, probe.register, step.targetType)
                            emitter.cast(slotValue, step.targetType)
                            emitter.goto(slotDone)
                            emitter.label(next)
                        }
                        emitter.label(slotDone)
                        emitter.ifZero(slotValue, nullLabel)
                        slotValue
                    }
                }
        }

        emitter.moveValue(result, current.register, path.resultType)
        emitter.goto(doneLabel)
        emitter.label(nullLabel)
        emitter.constZero(result, path.resultType)
        emitter.label(doneLabel)
        return emitter.Value(result, path.resultType)
    }

    private fun nullChecked(
        emitter: CodeEmitter,
        value: CodeEmitter.Value,
        nullLabel: EmissionLabel,
    ): CodeEmitter.Value {
        if (!isReferenceType(value.type)) return value
        val checked = value.asByte()
        emitter.ifZero(checked, nullLabel)
        return checked
    }

    private fun adaptInput(
        emitter: CodeEmitter,
        start: CodeEmitter.Value,
        rootType: String,
        label: String,
    ): CodeEmitter.Value {
        if (!isReferenceType(start.type) || !isReferenceType(rootType) || start.type == rootType)
            return start
        if (start.type == Type.Object) return emitter.cast(start, rootType).value(emitter)
        check(isAssignableType(start.type, rootType)) {
            "$label expects a value statically assignable to $rootType, got ${start.type}. " +
                "Only Object-typed inputs get the root check-cast implicitly; cast the value first."
        }
        return start
    }
}

private fun ValueRef.emitter(): CodeEmitter =
    (this as? CodeEmitter.Value)?.emitter
        ?: error("Bindings can only be applied inside a code block")

private fun ValueRef.value(emitter: CodeEmitter): CodeEmitter.Value = with(emitter) { impl() }
