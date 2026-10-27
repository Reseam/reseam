// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.emit

import app.reseam.patch.ExtMethod
import app.reseam.patch.FieldTarget
import app.reseam.patch.MethodTarget
import app.reseam.patch.Type
import app.reseam.patch.ValueRef
import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isStatic
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.method
import app.reseam.patch.run.ActiveRuntime
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class Value(
    val emitter: CodeEmitter,
    val register: Register,
    override val type: String,
    internal val source: FieldRef? = null,
    assignableType: ValueType = ValueType.Known(type),
) : ValueRef {
    constructor(
        emitter: CodeEmitter,
        register: Int,
        type: String,
    ) : this(emitter, Register.Physical(register), type)

    var assignableType: ValueType = assignableType
        private set

    val wordCount: Int = registerWordCount(type)

    fun registerWords(): List<Register> =
        (0 until wordCount).map { word ->
            when (register) {
                is Register.Physical -> Register.Physical(register.index + word)
                is Register.Temporary -> register.copy(word = register.word + word)
                Register.Void -> error("a void value has no register")
            }
        }

    fun asLow(): Value = fitted(RegisterConstraint.LOW)

    fun asByte(): Value = fitted(RegisterConstraint.BYTE)

    private fun fitted(constraint: RegisterConstraint): Value {
        val allocation =
            (register as? Register.Temporary)?.let {
                emitter.tempAllocations.getValue(it.allocation)
            }
        val incoming = allocation?.incomingOffset
        if (
            incoming != null &&
                emitter.incomingBase + incoming + wordCount - 1 <= constraint.maxRegister
        ) {
            if (constraint.maxRegister < allocation.constraint.maxRegister)
                allocation.constraint = constraint
            return this
        }
        if (emitter.registerFits(register, wordCount, constraint)) return this
        if (allocation != null && allocation.entryOffset == null && incoming == null) {
            allocation.constraint = constraint
            return this
        }
        emitter.lowCopies[register]
            ?.takeIf { it.type == type && emitter.registerFits(it.register, wordCount, constraint) }
            ?.let {
                return it
            }
        val dest = emitter.allocTemp(wordCount, constraint)
        emitter.moveValue(dest, register, type)
        return Value(emitter, dest, type, assignableType = assignableType).also {
            if (allocation == null || incoming != null) emitter.lowCopies[register] = it
        }
    }

    override fun assign(value: ValueRef) {
        require(source == null) {
            "cannot assign to the value read from ${source!!.definingClass}->${source.name}: it is a copy; " +
                "write the field with set(field, value) or setStatic(field, value)"
        }
        val newValue = value.builtBy(emitter)
        requireAssignableType(newValue.assignableType, type, "assignment")
        emitter.lowCopies.remove(register)
        emitter.moveValue(register, newValue.register, type)
        assignableType = ValueType.Known(type)
    }

    override fun cast(type: String): ValueRef = emitter.cast(this, descriptorOf(type))

    override fun field(field: FieldTarget): ValueRef = emitter.readField(this, field.ref)

    override fun field(ref: FieldRef): ValueRef = emitter.readField(this, ref)

    override fun fieldOfType(type: String): ValueRef =
        emitter.readField(
            this,
            ActiveRuntime.current.index
                .requireClass(this.type)
                .instanceFieldOfType(descriptorOf(type)),
        )

    override fun set(field: FieldTarget, value: ValueRef) =
        emitter.writeField(this, field.ref, value.builtBy(emitter))

    override fun set(ref: FieldRef, value: ValueRef) =
        emitter.writeField(this, ref, value.builtBy(emitter))

    override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
        require(!method.isStatic) { "$method is static; call it without a receiver" }
        method.target.method
        return emitter.invoke(
            Opcode.INVOKE_VIRTUAL,
            method.ref,
            listOf(this) + args.map { it.builtBy(emitter) },
        )
    }

    override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
        require(!method.method.isStatic) {
            "${method.descriptor} is static; call it without a receiver"
        }
        return emitter.invoke(
            emitter.instanceInvokeKind(method),
            method.ref,
            listOf(this) + args.map { it.builtBy(emitter) },
        )
    }

    override fun callVirtual(
        owner: String,
        name: String,
        proto: String,
        vararg args: ValueRef,
    ): ValueRef =
        emitter.invoke(
            Opcode.INVOKE_VIRTUAL,
            MethodRef(descriptorOf(owner), name, proto),
            listOf(this) + args.map { it.builtBy(emitter) },
        )

    override fun callInterface(
        owner: String,
        name: String,
        proto: String,
        vararg args: ValueRef,
    ): ValueRef =
        emitter.invoke(
            Opcode.INVOKE_INTERFACE,
            MethodRef(descriptorOf(owner), name, proto),
            listOf(this) + args.map { it.builtBy(emitter) },
        )

    override fun size(): ValueRef =
        emitter.invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "size", "()I"), listOf(this))

    override fun get(index: ValueRef): ValueRef =
        emitter.invoke(
            Opcode.INVOKE_INTERFACE,
            MethodRef(Type.List, "get", "(I)Ljava/lang/Object;"),
            listOf(this, index.builtBy(emitter)),
        )

    override fun plus(other: ValueRef): ValueRef =
        emitter.arithmetic(Opcode.ADD_INT, this, other.builtBy(emitter))

    override fun minus(other: ValueRef): ValueRef =
        emitter.arithmetic(Opcode.SUB_INT, this, other.builtBy(emitter))
}

/** This value as [emitter] built it; a value of another block, or of none, is rejected. */
internal fun ValueRef.builtBy(emitter: CodeEmitter): Value {
    val value =
        this as? Value ?: error("ValueRef is only valid inside the code block it was created in")
    require(value.emitter === emitter) { "ValueRef belongs to a different code block" }
    return value
}
