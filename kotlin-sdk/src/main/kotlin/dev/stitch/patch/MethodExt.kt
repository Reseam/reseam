@file:Suppress("unused")

package dev.stitch.patch

val MethodInfo.returnType: String
    get() = proto.substringAfterLast(")")

val MethodRef.returnType: String
    get() = proto.substringAfterLast(")")

fun Method.returnEarlyString(value: String) {
    addInstructions(0) {
        constString(0, value)
        returnObject(0)
    }
}

fun Method.indexOfOpcodeSequence(vararg opcodes: Int, start: Int = 0): Int? {
    val insns = instructions
    for (i in start..(insns.size - opcodes.size)) {
        var matched = true
        for (j in opcodes.indices) {
            if (insns[i + j].opcode() != opcodes[j]) {
                matched = false
                break
            }
        }
        if (matched) return i
    }
    return null
}

fun Method.indexOfFirstMethodCall(
    definingClass: String,
    methodName: String,
    start: Int = 0,
): Int? {
    val insns = instructions
    for (i in start until insns.size) {
        val ref = insns[i].methodRef() ?: continue
        if (ref.definingClass == definingClass && ref.name == methodName) return i
    }
    return null
}

fun Method.indexOfFirstFieldAccess(
    opcode: Int,
    fieldType: String? = null,
    definingClass: String? = null,
    start: Int = 0,
): Int? {
    val insns = instructions
    for (i in start until insns.size) {
        if (insns[i].opcode() != opcode) continue
        val ref = insns[i].fieldRef() ?: continue
        if (fieldType != null && ref.fieldType != fieldType) continue
        if (definingClass != null && ref.definingClass != definingClass) continue
        return i
    }
    return null
}

fun Method.insertLiteralOverride(
    literalIndex: Int,
    className: String,
    methodName: String,
    proto: String,
) {
    val moveResultIndex = indexOfFirst(Opcodes.MOVE_RESULT, literalIndex)
        ?: error("no MOVE_RESULT after literal at index $literalIndex")
    val register = registerA(moveResultIndex)
    insertInvokeStaticWithMoveResult(
        moveResultIndex + 1,
        className, methodName, proto,
        listOf(register), register, isObject = false,
    )
}

fun replaceConstString(hit: InstructionHit, newValue: String) {
    val method = Method(hit.method)
    val register = method.registerA(hit.index.toInt())
    method.replaceInstruction(
        hit.index.toInt(),
        Instruction.RegString(
            RegStringInsn(Opcodes.CONST_STRING.toUShort(), register.toUShort(), newValue)
        ),
    )
}
