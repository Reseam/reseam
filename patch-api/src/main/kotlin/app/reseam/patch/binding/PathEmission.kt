// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.binding

import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.emit.CodeEmitter
import app.reseam.patch.emit.Label
import app.reseam.patch.emit.Value
import app.reseam.patch.emit.builtBy
import app.reseam.patch.emit.cast
import app.reseam.patch.emit.constZero
import app.reseam.patch.emit.goto
import app.reseam.patch.emit.ifNonZero
import app.reseam.patch.emit.ifZero
import app.reseam.patch.emit.instanceOf
import app.reseam.patch.emit.invoke
import app.reseam.patch.emit.move
import app.reseam.patch.emit.readField
import app.reseam.patch.native.isAssignable
import app.reseam.patch.types.MethodRef

private fun ValueRef.emitter(): CodeEmitter =
    (this as? Value)?.emitter ?: error("Bindings can only be applied inside a code block")

internal fun ResolvedPath.emit(base: ValueRef, label: String): ValueRef {
    val emitter = base.emitter()
    val start = base.builtBy(emitter)
    var current = adaptInput(emitter, start, rootType, label)
    if (steps.isEmpty()) return current

    val nullLabel = emitter.label()
    val doneLabel = emitter.label()
    val result = emitter.temporary(resultType)

    for (step in steps) {
        current =
            when (step) {
                is PathStep.FieldRead ->
                    emitter
                        .readField(nullChecked(emitter, current, nullLabel), step.field)
                        .builtBy(emitter)
                is PathStep.VirtualCall ->
                    emitter
                        .invoke(
                            Opcode.INVOKE_VIRTUAL,
                            step.ref,
                            listOf(nullChecked(emitter, current, nullLabel)),
                        )
                        .builtBy(emitter)
                is PathStep.InterfaceCall ->
                    emitter
                        .invoke(
                            Opcode.INVOKE_INTERFACE,
                            step.ref,
                            listOf(nullChecked(emitter, current, nullLabel)),
                        )
                        .builtBy(emitter)
                PathStep.ListFirst -> {
                    emitter.ifZero(current, nullLabel)
                    val empty =
                        emitter
                            .invoke(
                                Opcode.INVOKE_INTERFACE,
                                MethodRef(Type.List, "isEmpty", "()Z"),
                                listOf(current),
                            )
                            .builtBy(emitter)
                    emitter.ifNonZero(empty, nullLabel)
                    current.get(emitter.code.int(0)).builtBy(emitter)
                }
                PathStep.ListLast -> {
                    emitter.ifZero(current, nullLabel)
                    current.get(current.size() - emitter.code.int(1)).builtBy(emitter)
                }
                is PathStep.CastValue -> emitter.cast(current, step.type).builtBy(emitter)
                is PathStep.ObjectSlot -> {
                    val obj = nullChecked(emitter, current, nullLabel)
                    val slotDone = emitter.label()
                    val slot = emitter.temporary(step.targetType)
                    emitter.constZero(slot)
                    for (field in step.fields) {
                        val next = emitter.label()
                        val probe = emitter.readField(obj, field).builtBy(emitter)
                        val isInstance = emitter.instanceOf(probe, step.targetType)
                        emitter.ifZero(isInstance, next)
                        emitter.move(slot, probe)
                        emitter.cast(slot, step.targetType)
                        emitter.goto(slotDone)
                        emitter.place(next)
                    }
                    emitter.place(slotDone)
                    emitter.ifZero(slot, nullLabel)
                    slot
                }
            }
    }

    emitter.move(result, current)
    emitter.goto(doneLabel)
    emitter.place(nullLabel)
    emitter.constZero(result)
    emitter.place(doneLabel)
    return result
}

private fun nullChecked(
    emitter: CodeEmitter,
    value: Value,
    nullLabel: Label,
): Value {
    if (isReferenceType(value.type)) emitter.ifZero(value, nullLabel)
    return value
}

private fun adaptInput(
    emitter: CodeEmitter,
    start: Value,
    rootType: String,
    label: String,
): Value {
    if (!isReferenceType(start.type) || !isReferenceType(rootType) || start.type == rootType)
        return start
    if (start.type == Type.Object) return emitter.cast(start, rootType).builtBy(emitter)
    check(isAssignable(start.type, rootType)) {
        "$label expects a value statically assignable to $rootType, got ${start.type}. " +
            "Only Object-typed inputs get the root check-cast implicitly; cast the value first."
    }
    return start
}
