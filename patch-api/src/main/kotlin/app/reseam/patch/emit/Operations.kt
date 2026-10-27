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
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.method
import app.reseam.patch.native.lowerInstruction
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.InvokeInsn
import app.reseam.patch.types.MethodRef

internal fun CodeEmitter.invoke(opcode: Opcode, ref: MethodRef, args: List<Value>): ValueRef {
    emitInvoke(opcode, ref, args)
    val returnType = ref.returnType
    if (returnType == Type.Void) return Value(this, Register.Void, Type.Void)
    val dest = allocTemp(registerWordCount(returnType))
    op(writes = listOf(dest)) { b, r ->
        b.moveResultTyped(byte(r(dest), "move-result"), returnType)
    }
    return Value(this, dest, returnType)
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
    val registers = args.flatMap { it.registerWords() }
    maxOutRegisters = maxOf(maxOutRegisters, registers.size)
    val invoke = InvokeAllocation(opcode, ref, registers)
    invokes += invoke
    invoke.operation =
        op(reads = registers) { b, r ->
            val resolved = registers.map(r)
            val input =
                Instruction.Invoke(
                    InvokeInsn(
                        opcode.value.toUShort(),
                        UShortArray(resolved.size) { resolved[it].toUShort() },
                        ref,
                    )
                )
            val span =
                invoke.scratch?.let { first ->
                    UShortArray(resolved.size) { (r(first) + it).toUShort() }
                } ?: ushortArrayOf()
            lowerInstruction(input, span).forEach(b::add)
        }
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
    val dest = allocTemp(registerWordCount(field.fieldType), RegisterConstraint.LOW)
    val obj = value.asLow()
    op(reads = listOf(obj.register), writes = listOf(dest)) { b, r ->
        b.igetTyped(low(r(dest), "iget A"), low(r(obj.register), "iget B"), field)
    }
    return Value(this, dest, field.fieldType, source = field)
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
    val src = newValue.asLow()
    val obj = value.asLow()
    op(reads = listOf(src.register, obj.register)) { b, r ->
        b.iputTyped(low(r(src.register), "iput A"), low(r(obj.register), "iput B"), field)
    }
}

internal fun CodeEmitter.arithmetic(opcode: Opcode, left: Value, right: Value): ValueRef {
    requireArgumentTypes(
        listOf(left.assignableType, right.assignableType),
        listOf(Type.Int, Type.Int),
        "$opcode operands",
    )
    val dest = allocTemp()
    val a = left.asByte()
    val c = right.asByte()
    op(reads = listOf(a.register, c.register), writes = listOf(dest)) { b, r ->
        b.reg3(
            opcode,
            byte(r(dest), "binop A"),
            byte(r(a.register), "binop B"),
            byte(r(c.register), "binop C"),
        )
    }
    return Value(this, dest, Type.Int)
}

internal fun CodeEmitter.cast(value: Value, type: String): ValueRef {
    require(
        (isReferenceType(value.type) || value.assignableType == ValueType.Zero) &&
            isReferenceType(type)
    ) {
        "check-cast requires reference types"
    }
    access.requireClass(type)
    val target = value.asByte()
    op(reads = listOf(target.register), writes = listOf(target.register)) { b, r ->
        b.checkCast(byte(r(target.register), "check-cast"), type)
    }
    return Value(
        this,
        target.register,
        type,
        assignableType =
            when (value.assignableType) {
                ValueType.Null,
                ValueType.Zero -> ValueType.Null
                is ValueType.Known -> ValueType.Known(type)
            },
    )
}

internal fun CodeEmitter.instanceOf(value: Value, type: String): Value {
    requireAssignableType(value.assignableType, Type.Object, "instance-of operand")
    require(isReferenceType(type)) { "instance-of requires a reference type" }
    access.requireClass(type)
    val dest = allocTemp(constraint = RegisterConstraint.LOW)
    val ref = value.asLow()
    op(reads = listOf(ref.register), writes = listOf(dest)) { b, r ->
        b.instanceOf(low(r(dest), "instance-of A"), low(r(ref.register), "instance-of B"), type)
    }
    return Value(this, dest, Type.Boolean)
}

internal fun CodeEmitter.constZero(dest: Register, type: String) {
    op(writes = listOf(dest)) { b, r ->
        if (registerWordCount(type) == 2) b.constLong(byte(r(dest), "const-wide"), 0)
        else b.constInt(byte(r(dest), "const"), 0)
    }
}

internal fun CodeEmitter.moveValue(dest: Register, src: Register, type: String) {
    op(reads = listOf(src), writes = listOf(dest)) { b, r ->
        b.moveTyped(r(dest), r(src), type)
    }
}

internal fun CodeEmitter.readStatic(field: FieldRef): Value {
    access.requireField(field)
    val dest = allocTemp(registerWordCount(field.fieldType))
    op(writes = listOf(dest)) { b, r -> b.sgetTyped(byte(r(dest), "sget"), field) }
    return Value(this, dest, field.fieldType, source = field)
}

internal fun CodeEmitter.writeStatic(field: FieldRef, value: Value) {
    access.requireField(field)
    requireAssignableType(
        value.assignableType,
        field.fieldType,
        "write ${field.definingClass}->${field.name}",
    )
    val src = value.asByte()
    op(reads = listOf(src.register)) { b, r -> b.sputTyped(byte(r(src.register), "sput"), field) }
}

/** A new instance of [owner], built by its constructor of [ctorProto]. */
internal fun CodeEmitter.construct(owner: String, ctorProto: String, args: List<Value>): Value {
    require(owner.startsWith('L') && owner.endsWith(';') && owner.length > 2) {
        "new-instance requires a class type: $owner"
    }
    val constructor = MethodRef(owner, "<init>", ctorProto)
    require(constructor.returnType == Type.Void) { "A constructor must return void: $ctorProto" }
    access.requireClass(owner)
    val dest = allocTemp()
    op(writes = listOf(dest)) { b, r -> b.newInstance(byte(r(dest), "new-instance"), owner) }
    val instance = Value(this, dest, owner)
    invoke(Opcode.INVOKE_DIRECT, constructor, listOf(instance) + args)
    return instance
}
