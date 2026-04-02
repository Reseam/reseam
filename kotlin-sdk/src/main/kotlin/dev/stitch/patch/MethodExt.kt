@file:Suppress("unused")

package dev.stitch.patch

val MethodInfo.returnType: String
    get() = proto.substringAfterLast(")")

val MethodRef.returnType: String
    get() = proto.substringAfterLast(")")

val MethodRef.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

val MethodInfo.parameterTypes: List<String>
    get() = parseParameterTypes(proto)

private fun parseParameterTypes(proto: String): List<String> {
    val params = proto.substringAfter("(").substringBefore(")")
    if (params.isEmpty()) return emptyList()
    val result = mutableListOf<String>()
    var i = 0
    while (i < params.length) {
        when (params[i]) {
            '[' -> {
                val start = i
                i++
                while (i < params.length && params[i] == '[') i++
                if (i < params.length && params[i] == 'L') {
                    val end = params.indexOf(';', i)
                    result.add(params.substring(start, end + 1))
                    i = end + 1
                } else if (i < params.length) {
                    result.add(params.substring(start, i + 1))
                    i++
                }
            }
            'L' -> {
                val end = params.indexOf(';', i)
                result.add(params.substring(i, end + 1))
                i = end + 1
            }
            else -> {
                result.add(params[i].toString())
                i++
            }
        }
    }
    return result
}

val Method.returnType: String
    get() = info.returnType

val Method.parameterTypes: List<String>
    get() = info.parameterTypes

fun Method.returnEarlyString(value: String) {
    addInstructions(0) {
        constString(0, value)
        returnObject(0)
    }
}

fun Method.indexOfOpcodeSequence(vararg opcodes: Int, start: Int = 0): Int? =
    indexOfOpcodeSequence(opcodes, start)

fun Method.indexOfFirstMethodCall(
    definingClass: String,
    methodName: String,
    start: Int = 0,
): Int? = indexOfFirstMethodCall(definingClass, methodName, start)

fun Method.indexOfFirstFieldAccess(
    opcode: Int,
    fieldType: String? = null,
    definingClass: String? = null,
    start: Int = 0,
): Int? = indexOfFirstFieldAccess(opcode, fieldType, definingClass, start)

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

fun Method.indexOfFirstInstruction(startIndex: Int = 0, filter: Instruction.() -> Boolean): Int {
    val insns = instructions
    for (i in startIndex until insns.size) {
        if (filter(insns[i])) return i
    }
    return -1
}

fun Method.indexOfFirstInstructionReversed(startIndex: Int? = null, filter: Instruction.() -> Boolean): Int {
    val insns = instructions
    val start = startIndex ?: (insns.size - 1)
    for (i in start downTo 0) {
        if (filter(insns[i])) return i
    }
    return -1
}

fun Method.addInstructionsAtControlFlowLabel(insertIndex: Int, block: InstructionBuilder.() -> Unit) {
    val original = instructions[insertIndex]
    insertInstruction(insertIndex + 1, original)
    insertInstructions(insertIndex + 1, buildInstructions(block))
    removeInstruction(insertIndex)
}

fun Method.containsLiteralInstruction(literal: Long): Boolean =
    containsLiteral(literal)

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
