// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.descriptor as descriptorOf
import app.reseam.patch.dex.AccessFlags
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.Opcode
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.isReferenceType
import app.reseam.patch.dex.isSet
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.registerWordCount
import app.reseam.patch.dex.returnType
import app.reseam.patch.types.FieldRef
import app.reseam.patch.types.MethodRef

internal class CodeEmitter
private constructor(
    internal val method: Method,
    internal val insertIndex: Int?,
    internal val mode: EmissionMode,
    internal val captures: List<Capture>,
    internal val entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
) : CodeScope {
    internal val ops = mutableListOf<Op>()
    internal val usedRegisters = mutableSetOf<Int>()
    internal var nextTempId = 0
    internal var plannedLocalGrowth = 0
    internal var replacementLocals = MIN_REPLACEMENT_LOCALS
    internal var maxOutRegisters = 0
    internal var labelCounter = 0
    internal val tempAllocations = mutableMapOf<TemporaryId, TempAllocation>()
    internal val entryValues = mutableMapOf<Int, Value>()
    internal val invokes = mutableListOf<InvokeAllocation>()
    internal val lowCopies = mutableMapOf<Register, Value>()

    companion object {
        fun forInsertion(
            method: Method,
            index: Int,
            captures: List<Capture>,
            entrySnapshots: MutableMap<Int, EntrySnapshot>? = null,
        ) =
            CodeEmitter(
                method,
                index,
                mode = EmissionMode.INSERTION,
                captures = captures,
                entrySnapshots = entrySnapshots,
            )

        fun forReplacement(method: Method) =
            CodeEmitter(
                method,
                insertIndex = null,
                mode = EmissionMode.REPLACEMENT,
                captures = emptyList(),
            )
    }

    internal val info = method.info
    internal val access = Access(info.classDescriptor)
    internal val reservedLocals =
        if (mode == EmissionMode.REPLACEMENT) emptyList()
        else ActiveRuntime.current.edits.reserved(method)
    internal val isStaticMethod = AccessFlags.STATIC.isSet(info.accessFlags)
    internal val incomingWords =
        if (mode == EmissionMode.REPLACEMENT)
            method.parameterTypes.sumOf(::registerWordCount) + if (isStaticMethod) 0 else 1
        else method.insSize
    internal val originalRegistersSize =
        if (mode == EmissionMode.REPLACEMENT) MIN_REPLACEMENT_LOCALS + incomingWords
        else method.registersSize
    internal val incomingBase =
        if (mode == EmissionMode.REPLACEMENT) MIN_REPLACEMENT_LOCALS
        else method.registersSize - incomingWords

    override val thisObject: ValueRef
        get() {
            require(!isStaticMethod) { "thisObject is not available in static ${info.descriptor}" }
            return incomingValue(0, info.classDescriptor)
        }

    override fun param(index: Int): ValueRef {
        val params = method.parameterTypes
        require(index in params.indices) {
            "Parameter index $index out of bounds for ${info.descriptor}"
        }
        val offset = (if (isStaticMethod) 0 else 1) + params.take(index).sumOf(::registerWordCount)
        return incomingValue(offset, params[index])
    }

    override fun paramOfType(type: String): ValueRef {
        val wanted = descriptorOf(type)
        val index = method.parameterTypes.indexOf(wanted)
        require(index >= 0) { "No parameter of type $wanted in ${info.descriptor}" }
        return param(index)
    }

    override val lastParam: ValueRef
        get() = param(method.parameterTypes.lastIndex)

    override fun capture(name: String): ValueRef {
        val capture =
            captures.firstOrNull { it.name == name }
                ?: error(
                    "No capture named '$name' here; available: ${captures.joinToString { it.name }.ifEmpty { "none" }}"
                )
        return Value(capture.register, capture.type)
    }

    override fun local(slot: MethodLocal): ValueRef {
        require(mode != EmissionMode.REPLACEMENT) {
            "$slot is a register of the original body, which this block replaces"
        }
        require(!slot.lost) {
            "$slot no longer exists: the method body was replaced after it was reserved"
        }
        require(slot.method.handle == method.handle) {
            "$slot is a local of another method, not ${info.descriptor}"
        }
        return Value(slot.register, slot.type)
    }

    override fun int(value: Int): ValueRef {
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.constInt(byte(r(dest), "const"), value) }
        return Value(
            dest,
            Type.Int,
            assignableType = if (value == 0) ValueType.Zero else ValueType.Known(Type.Int),
        )
    }

    override fun long(value: Long): ValueRef {
        val dest = allocTemp(wordCount = 2)
        op(writes = listOf(dest)) { b, r -> b.constLong(byte(r(dest), "const-wide"), value) }
        return Value(dest, Type.Long)
    }

    override fun bool(value: Boolean): ValueRef {
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r ->
            b.constInt(byte(r(dest), "const"), if (value) 1 else 0)
        }
        return Value(dest, Type.Boolean)
    }

    override fun string(value: String): ValueRef {
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.constString(byte(r(dest), "const-string"), value) }
        return Value(dest, Type.String)
    }

    override val nullObject: ValueRef
        get() {
            val dest = allocTemp()
            op(writes = listOf(dest)) { b, r -> b.constInt(byte(r(dest), "const"), 0) }
            return Value(dest, Type.Object, assignableType = ValueType.Null)
        }

    override fun enumValue(type: String, name: String): ValueRef {
        val owner = descriptorOf(type)
        return staticField(FieldRef(owner, name, owner))
    }

    override fun staticField(field: FieldTarget): ValueRef = staticField(field.ref)

    override fun staticField(field: FieldRef): ValueRef {
        access.requireField(field)
        val dest = allocTemp(registerWordCount(field.fieldType))
        op(writes = listOf(dest)) { b, r -> b.sgetTyped(byte(r(dest), "sget"), field) }
        return Value(dest, field.fieldType, source = field)
    }

    override fun setStatic(field: FieldTarget, value: ValueRef) = setStatic(field.ref, value)

    override fun setStatic(field: FieldRef, value: ValueRef) {
        access.requireField(field)
        val supplied = value.impl()
        requireAssignableType(
            supplied.assignableType,
            field.fieldType,
            "write ${field.definingClass}->${field.name}",
        )
        val src = supplied.asByte()
        op(reads = listOf(src.register)) { b, r ->
            b.sputTyped(byte(r(src.register), "sput"), field)
        }
    }

    override fun newInstance(type: String, ctorProto: String, vararg args: ValueRef): ValueRef {
        val owner = descriptorOf(type)
        require(owner.startsWith('L') && owner.endsWith(';') && owner.length > 2) {
            "new-instance requires a class type: $owner"
        }
        val constructor = MethodRef(owner, "<init>", ctorProto)
        require(constructor.returnType == Type.Void) {
            "A constructor must return void: $ctorProto"
        }
        access.requireClass(owner)
        val dest = allocTemp()
        op(writes = listOf(dest)) { b, r -> b.newInstance(byte(r(dest), "new-instance"), owner) }
        val instance = Value(dest, owner)
        invoke(
            Opcode.INVOKE_DIRECT,
            constructor,
            listOf(instance) + args.map { it.impl() },
        )
        return instance
    }

    override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
        require(method.isStatic) {
            "$method is an instance method; call it on a value: receiver.call(...)"
        }
        method.target.method
        return invoke(Opcode.INVOKE_STATIC, method.ref, args.map { it.impl() })
    }

    override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
        require(method.method.isStatic) {
            "${method.descriptor} is an instance method; call it on a value: receiver.call(...)"
        }
        return invoke(Opcode.INVOKE_STATIC, method.ref, args.map { it.impl() })
    }

    override fun callStatic(
        owner: String,
        name: String,
        proto: String,
        vararg args: ValueRef,
    ): ValueRef =
        invoke(
            Opcode.INVOKE_STATIC,
            MethodRef(descriptorOf(owner), name, proto),
            args.map { it.impl() },
        )

    override fun whenTrue(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val register =
            value.impl().also { requireZeroComparable(it.assignableType) }.asByte().register
        return branch(block) { elseLabel ->
            op(reads = listOf(register), target = elseLabel) { b, r ->
                b.ifEqz(byte(r(register), "if-eqz"), elseLabel.name)
            }
        }
    }

    override fun whenFalse(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        val register =
            value.impl().also { requireZeroComparable(it.assignableType) }.asByte().register
        return branch(block) { elseLabel ->
            op(reads = listOf(register), target = elseLabel) { b, r ->
                b.ifNez(byte(r(register), "if-nez"), elseLabel.name)
            }
        }
    }

    override fun whenNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        requireAssignableType(value.impl().assignableType, Type.Object, "null comparison")
        return whenFalse(value, block)
    }

    override fun whenNotNull(value: ValueRef, block: CodeScope.() -> Unit): Otherwise {
        requireAssignableType(value.impl().assignableType, Type.Object, "null comparison")
        return whenTrue(value, block)
    }

    override fun whenEqual(
        left: ValueRef,
        right: ValueRef,
        block: CodeScope.() -> Unit,
    ): Otherwise = equality(left.impl(), right.impl(), Equality.EQUAL, block)

    override fun whenNotEqual(
        left: ValueRef,
        right: ValueRef,
        block: CodeScope.() -> Unit,
    ): Otherwise = equality(left.impl(), right.impl(), Equality.NOT_EQUAL, block)

    override fun whenInstanceOf(
        value: ValueRef,
        type: String,
        block: CodeScope.() -> Unit,
    ): Otherwise {
        val isInstance = instanceOf(value.impl(), descriptorOf(type)).register
        return branch(block) { elseLabel ->
            op(reads = listOf(isInstance), target = elseLabel) { b, r ->
                b.ifEqz(byte(r(isInstance), "if-eqz"), elseLabel.name)
            }
        }
    }

    override fun returnVoid() {
        requireAssignableType(Type.Void, info.returnType, "return from ${info.descriptor}")
        op(fallsThrough = false) { b, _ -> b.returnVoid() }
    }

    override fun returnValue(value: ValueRef) {
        val supplied = value.impl()
        requireAssignableType(
            supplied.assignableType,
            info.returnType,
            "return from ${info.descriptor}",
        )
        require(info.returnType != Type.Void) { "${info.descriptor} returns no value" }
        val impl = supplied.asByte()
        val type = info.returnType
        op(reads = listOf(impl.register), fallsThrough = false) { b, r ->
            val register = byte(r(impl.register), "return")
            when {
                isReferenceType(type) -> b.returnObject(register)
                registerWordCount(type) == 2 -> b.returnWide(register)
                else -> b.returnValue(register)
            }
        }
    }

    override fun returnTrue() = returnValue(bool(true))

    override fun returnFalse() = returnValue(bool(false))

    override fun returnNull() = returnValue(nullObject)

    internal inner class Value(
        val register: Register,
        override val type: String,
        internal val source: FieldRef? = null,
        assignableType: ValueType = ValueType.Known(type),
    ) : ValueRef {
        constructor(register: Int, type: String) : this(Register.Physical(register), type)

        var assignableType: ValueType = assignableType
            private set

        val wordCount: Int = registerWordCount(type)
        val emitter: CodeEmitter
            get() = this@CodeEmitter

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
            if (register is Register.Temporary) {
                val allocation = tempAllocations.getValue(register.allocation)
                val incoming = allocation.incomingOffset
                if (
                    incoming != null &&
                        incomingBase + incoming + wordCount - 1 <= constraint.maxRegister
                ) {
                    if (constraint.maxRegister < allocation.constraint.maxRegister)
                        allocation.constraint = constraint
                    return this
                }
            }
            if (registerFits(register, wordCount, constraint)) return this
            if (register is Register.Temporary) {
                val allocation = tempAllocations.getValue(register.allocation)
                if (allocation.entryOffset == null && allocation.incomingOffset == null) {
                    allocation.constraint = constraint
                    return this
                }
            }
            lowCopies[register]
                ?.takeIf { it.type == type && registerFits(it.register, wordCount, constraint) }
                ?.let {
                    return it
                }
            val dest = allocTemp(wordCount, constraint)
            moveValue(dest, register, type)
            return Value(dest, type, assignableType = assignableType).also {
                if (
                    register !is Register.Temporary ||
                        tempAllocations.getValue(register.allocation).incomingOffset != null
                )
                    lowCopies[register] = it
            }
        }

        override fun assign(value: ValueRef) {
            require(source == null) {
                "cannot assign to the value read from ${source!!.definingClass}->${source.name}: it is a copy; " +
                    "write the field with set(field, value) or setStatic(field, value)"
            }
            val newValue = value.impl()
            requireAssignableType(newValue.assignableType, type, "assignment")
            lowCopies.remove(register)
            moveValue(register, newValue.register, type)
            assignableType = ValueType.Known(type)
        }

        override fun cast(type: String): ValueRef = cast(this, descriptorOf(type))

        override fun field(field: FieldTarget): ValueRef = readField(this, field.ref)

        override fun field(ref: FieldRef): ValueRef = readField(this, ref)

        override fun fieldOfType(type: String): ValueRef =
            readField(this, uniqueField(this.type, descriptorOf(type)))

        override fun set(field: FieldTarget, value: ValueRef) =
            writeField(this, field.ref, value.impl())

        override fun set(ref: FieldRef, value: ValueRef) = writeField(this, ref, value.impl())

        override fun call(method: ExtMethod, vararg args: ValueRef): ValueRef {
            require(!method.isStatic) { "$method is static; call it without a receiver" }
            method.target.method
            return invoke(Opcode.INVOKE_VIRTUAL, method.ref, listOf(this) + args.map { it.impl() })
        }

        override fun call(method: MethodTarget, vararg args: ValueRef): ValueRef {
            require(!method.method.isStatic) {
                "${method.descriptor} is static; call it without a receiver"
            }
            return invoke(
                instanceInvokeKind(method),
                method.ref,
                listOf(this) + args.map { it.impl() },
            )
        }

        override fun callVirtual(
            owner: String,
            name: String,
            proto: String,
            vararg args: ValueRef,
        ): ValueRef =
            invoke(
                Opcode.INVOKE_VIRTUAL,
                MethodRef(descriptorOf(owner), name, proto),
                listOf(this) + args.map { it.impl() },
            )

        override fun callInterface(
            owner: String,
            name: String,
            proto: String,
            vararg args: ValueRef,
        ): ValueRef =
            invoke(
                Opcode.INVOKE_INTERFACE,
                MethodRef(descriptorOf(owner), name, proto),
                listOf(this) + args.map { it.impl() },
            )

        override fun size(): ValueRef =
            invoke(Opcode.INVOKE_INTERFACE, MethodRef(Type.List, "size", "()I"), listOf(this))

        override fun get(index: ValueRef): ValueRef =
            invoke(
                Opcode.INVOKE_INTERFACE,
                MethodRef(Type.List, "get", "(I)Ljava/lang/Object;"),
                listOf(this, index.impl()),
            )

        override fun plus(other: ValueRef): ValueRef =
            arithmetic(Opcode.ADD_INT, this, other.impl())

        override fun minus(other: ValueRef): ValueRef =
            arithmetic(Opcode.SUB_INT, this, other.impl())
    }

    internal fun ValueRef.impl(): Value {
        val value =
            this as? Value
                ?: error("ValueRef is only valid inside the code block it was created in")
        require(value.emitter === this@CodeEmitter) { "ValueRef belongs to a different code block" }
        return value
    }
}
