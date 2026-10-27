// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.MethodTarget
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.method
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal fun CodeEmitter.invoke(opcode: Opcode, ref: MethodRef, args: List<Value>): ValueRef {
    emitInvoke(opcode, ref, args)
    val returnType = ref.returnType
    if (returnType == Type.Void) return Value(this, null, Type.Void)
    val result = temporary(returnType)
    emit(template { moveResult(0, returnType) }, result)
    return result
}

internal fun CodeEmitter.emitInvoke(
    opcode: Opcode,
    ref: MethodRef,
    args: List<Value>,
) {
    val expected =
        if (opcode == Opcode.INVOKE_STATIC || opcode == Opcode.INVOKE_STATIC_RANGE)
            ref.parameterTypes
        else listOf(ref.definingClass) + ref.parameterTypes
    requireArgumentTypes(args.map { it.assignableType }, expected, ref.descriptor)
    access.requireMethod(ref)
    val words = args.sumOf { it.wordCount }
    emit(
        template { invokeRange(opcode.rangeVariant ?: opcode, ref, 0, words) },
        *args.toTypedArray(),
    )
}

private val Opcode.rangeVariant: Opcode?
    get() =
        when (this) {
            Opcode.INVOKE_VIRTUAL -> Opcode.INVOKE_VIRTUAL_RANGE
            Opcode.INVOKE_SUPER -> Opcode.INVOKE_SUPER_RANGE
            Opcode.INVOKE_DIRECT -> Opcode.INVOKE_DIRECT_RANGE
            Opcode.INVOKE_STATIC -> Opcode.INVOKE_STATIC_RANGE
            Opcode.INVOKE_INTERFACE -> Opcode.INVOKE_INTERFACE_RANGE
            else -> null
        }

internal fun CodeEmitter.instanceInvokeKind(target: MethodTarget): Opcode {
    val info = target.method.info
    return when {
        AccessFlags.PRIVATE.isSet(info.accessFlags) ||
            AccessFlags.CONSTRUCTOR.isSet(info.accessFlags) -> Opcode.INVOKE_DIRECT
        ActiveRuntime.current.index.classFor(target.owner)?.isInterface == true ->
            Opcode.INVOKE_INTERFACE
        else -> Opcode.INVOKE_VIRTUAL
    }
}

internal fun CodeEmitter.readField(value: Value, field: FieldRef): ValueRef {
    requireAssignableType(
        value.assignableType,
        field.definingClass,
        "read ${field.name} receiver",
    )
    access.requireField(field)
    val result = temporary(field.fieldType, source = field)
    emit(template { iget(0, 0, field) }, result, value)
    return result
}

internal fun CodeEmitter.writeField(value: Value, field: FieldRef, newValue: Value) {
    requireAssignableType(
        value.assignableType,
        field.definingClass,
        "write ${field.name} receiver",
    )
    requireAssignableType(
        newValue.assignableType,
        field.fieldType,
        "write ${field.definingClass}->${field.name}",
    )
    access.requireField(field)
    emit(template { iput(0, 0, field) }, newValue, value)
}

internal fun CodeEmitter.arithmetic(opcode: Opcode, left: Value, right: Value): ValueRef {
    requireArgumentTypes(
        listOf(left.assignableType, right.assignableType),
        listOf(Type.Int, Type.Int),
        "$opcode operands",
    )
    val result = temporary(Type.Int)
    emit(template { reg3(opcode, 0, 0, 0) }, result, left, right)
    return result
}

internal fun CodeEmitter.cast(value: Value, type: String): ValueRef {
    require(
        (isReferenceType(value.type) || value.assignableType == ValueType.Zero) &&
            isReferenceType(type)
    ) {
        "check-cast requires reference types"
    }
    access.requireClass(type)
    // check-cast narrows the register it runs on, so cast a copy: a parameter or capture keeps
    // its type for the method code around the block.
    val cast =
        temporary(
            type,
            assignableType =
                when (value.assignableType) {
                    ValueType.Null,
                    ValueType.Zero -> ValueType.Null
                    is ValueType.Known -> ValueType.Known(type)
                },
        )
    move(cast, value)
    emit(template { checkCast(0, type) }, cast)
    return cast
}

internal fun CodeEmitter.instanceOf(value: Value, type: String): Value {
    requireAssignableType(value.assignableType, Type.Object, "instance-of operand")
    require(isReferenceType(type)) { "instance-of requires a reference type" }
    access.requireClass(type)
    val result = temporary(Type.Boolean)
    emit(template { instanceOf(0, 0, type) }, result, value)
    return result
}

/** Sets [dest] to zero, or null for a reference. */
internal fun CodeEmitter.constZero(dest: Value) {
    emit(template { if (dest.wordCount == 2) constLong(0, 0) else constInt(0, 0) }, dest)
}

internal fun CodeEmitter.move(dest: Value, src: Value) {
    emit(template { move(0, 0, dest.type) }, dest, src)
}

internal fun CodeEmitter.readStatic(field: FieldRef): Value {
    access.requireField(field)
    val result = temporary(field.fieldType, source = field)
    emit(template { sget(0, field) }, result)
    return result
}

internal fun CodeEmitter.writeStatic(field: FieldRef, value: Value) {
    access.requireField(field)
    requireAssignableType(
        value.assignableType,
        field.fieldType,
        "write ${field.definingClass}->${field.name}",
    )
    emit(template { sput(0, field) }, value)
}

/** A new instance of [owner], built by its constructor of [ctorProto]. */
internal fun CodeEmitter.construct(owner: String, ctorProto: String, args: List<Value>): Value {
    require(owner.startsWith('L') && owner.endsWith(';') && owner.length > 2) {
        "new-instance requires a class type: $owner"
    }
    val constructor = MethodRef(owner, "<init>", ctorProto)
    require(constructor.returnType == Type.Void) { "A constructor must return void: $ctorProto" }
    access.requireClass(owner)
    val instance = temporary(owner)
    emit(template { newInstance(0, owner) }, instance)
    invoke(Opcode.INVOKE_DIRECT, constructor, listOf(instance) + args)
    return instance
}
