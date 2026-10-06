// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.CodeEmitter.Value
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.InstructionBuilder
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.lowerInstruction
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.Instruction
import app.reseam.patch.types.InvokeInsn
import app.reseam.patch.types.MethodRef

internal fun CodeEmitter.buildReplacement(): ReplacementPlan {
    // Trailing labels may otherwise leave branches targeting past the method body.
    val trailingLabels = ops.takeLastWhile { it.label != null }.mapNotNull { it.label }.toSet()
    check(
        ops.lastOrNull { it.label == null }?.fallsThrough == false &&
            ops.none { it.target in trailingLabels }
    ) {
        "The replacement for ${info.descriptor} falls off the end of the method; end every path with a return"
    }
    val instructions = build()
    return ReplacementPlan(replacementLocals + incomingWords, maxOutRegisters, instructions)
}

internal fun CodeEmitter.build(): List<Instruction> {
    layoutRegisters()
    val builder = InstructionBuilder()
    if (mode == EmissionMode.REPLACEMENT)
        for ((offset, value) in entryValues) {
            val allocation =
                tempAllocations.getValue((value.register as Register.Temporary).allocation)
            if (allocation.stageIncoming)
                builder.moveTyped(
                    checkNotNull(allocation.baseRegister),
                    replacementLocals + offset,
                    value.type,
                )
        }
    for (op in ops) op.emit(builder, ::resolveRegister)
    return builder.build()
}

internal fun CodeEmitter.invoke(opcode: Opcode, ref: MethodRef, args: List<Value>): ValueRef {
    emitInvoke(opcode, ref, args)
    val returnType = ref.returnType
    if (returnType == Type.Void) return Value(Register.Void, Type.Void)
    val dest = allocTemp(registerWordCount(returnType))
    op(writes = listOf(dest)) { b, r ->
        b.moveResultTyped(byte(r(dest), "move-result"), returnType)
    }
    return Value(dest, returnType)
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
    return Value(dest, field.fieldType, source = field)
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

internal fun CodeEmitter.uniqueField(owner: String, type: String): FieldRef {
    val classDef = ActiveRuntime.current.index.classFor(owner) ?: error("Class not found: $owner")
    val fields = classDef.instanceFields.filter { it.fieldType == type }
    require(fields.size == 1) {
        "Expected exactly one instance field of type $type on $owner, found ${fields.size}"
    }
    return FieldRef(owner, fields.single().name, type)
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
    return Value(dest, Type.Int)
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

internal fun CodeEmitter.label(name: EmissionLabel) {
    lowCopies.clear()
    op(label = name) { b, _ -> b.label(name.name) }
}

internal fun CodeEmitter.goto(label: EmissionLabel) {
    op(target = label, fallsThrough = false) { b, _ -> b.goto(label.name) }
}

internal fun CodeEmitter.ifZero(value: Value, label: EmissionLabel) {
    requireZeroComparable(value.assignableType)
    val v = value.asByte()
    op(reads = listOf(v.register), target = label) { b, r ->
        b.ifEqz(byte(r(v.register), "if-eqz"), label.name)
    }
}

internal fun CodeEmitter.ifNonZero(value: Value, label: EmissionLabel) {
    requireZeroComparable(value.assignableType)
    val v = value.asByte()
    op(reads = listOf(v.register), target = label) { b, r ->
        b.ifNez(byte(r(v.register), "if-nez"), label.name)
    }
}

internal fun CodeEmitter.constZero(dest: Register, type: String) {
    op(writes = listOf(dest)) { b, r ->
        if (registerWordCount(type) == 2) b.constLong(byte(r(dest), "const-wide"), 0)
        else b.constInt(byte(r(dest), "const"), 0)
    }
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
    return Value(dest, Type.Boolean)
}

internal fun CodeEmitter.nextLabel(): EmissionLabel = EmissionLabel(labelCounter++)

internal fun CodeEmitter.op(
    reads: List<Register> = emptyList(),
    writes: List<Register> = emptyList(),
    label: EmissionLabel? = null,
    target: EmissionLabel? = null,
    fallsThrough: Boolean = true,
    emit: Emit,
): Op = Op(reads, writes, label, target, fallsThrough, emit).also { ops += it }

internal fun CodeEmitter.allocTemp(
    wordCount: Int = 1,
    constraint: RegisterConstraint = RegisterConstraint.BYTE,
): Register.Temporary {
    require(wordCount > 0)
    val tempId = TemporaryId(nextTempId++)
    tempAllocations[tempId] = TempAllocation(wordCount, constraint)
    return Register.Temporary(tempId)
}

internal fun CodeEmitter.moveValue(dest: Register, src: Register, type: String) {
    op(reads = listOf(src), writes = listOf(dest)) { b, r ->
        b.moveTyped(r(dest), r(src), type)
    }
}

internal fun CodeEmitter.low(register: Int, context: String): Int {
    require(register in 0..15) { "$context requires a 4-bit register, got v$register" }
    return register
}

internal fun CodeEmitter.byte(register: Int, context: String): Int {
    require(register in 0..0xFF) { "$context requires an 8-bit register, got v$register" }
    return register
}
