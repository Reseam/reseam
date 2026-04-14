package dev.stitch.patch.settings

import dev.stitch.patch.InstructionBuilder
import dev.stitch.patch.Method
import dev.stitch.patch.returnType

private const val SETTINGS_CLASS = "Ldev/stitch/runtime/settings/StitchSettings;"

fun Method.prependWhen(
    setting: ToggleSetting,
    block: InstructionBuilder.() -> Unit,
) {
    val scratch = findFreeRegisters(0, 2)
    require(scratch.size >= 2 && scratch.all { it <= 15 }) {
        "Cannot insert setting check for '${setting.key}' in ${info.classDescriptor}->${info.methodName}: " +
            "no two free 4-bit scratch registers"
    }

    val keyReg = scratch[0]
    val defaultReg = scratch[1]
    if (outsSize < 2) setRegisters(registersSize, 2)

    addInstructions(0) {
        constString(keyReg, setting.key)
        const4(defaultReg, if (setting.default) 1 else 0)
        invokeStatic(SETTINGS_CLASS, "getBoolean", "(Ljava/lang/String;Z)Z", keyReg, defaultReg)
        moveResult(defaultReg)
        ifEqz(defaultReg, "stitch_setting_disabled")
        block()
        label("stitch_setting_disabled")
    }
}

fun Method.skipWhen(setting: ToggleSetting) {
    require(info.returnType == "V") {
        "skipWhen('${setting.key}') requires a void method, got ${info.returnType} in " +
            "${info.classDescriptor}->${info.methodName}"
    }
    prependWhen(setting) { returnVoid() }
}

fun Method.returnNullWhen(setting: ToggleSetting) {
    require(info.returnType.startsWith("L") || info.returnType.startsWith("[")) {
        "returnNullWhen('${setting.key}') requires an object/array method, got ${info.returnType} in " +
            "${info.classDescriptor}->${info.methodName}"
    }
    val scratch = findFreeRegisters(0, 3)
    require(scratch.size >= 3 && scratch.all { it <= 15 }) {
        "Cannot insert returnNullWhen('${setting.key}') in ${info.classDescriptor}->${info.methodName}: " +
            "no three free 4-bit scratch registers"
    }

    val keyReg = scratch[0]
    val defaultReg = scratch[1]
    val nullReg = scratch[2]
    if (outsSize < 2) setRegisters(registersSize, 2)

    addInstructions(0) {
        constString(keyReg, setting.key)
        const4(defaultReg, if (setting.default) 1 else 0)
        invokeStatic(SETTINGS_CLASS, "getBoolean", "(Ljava/lang/String;Z)Z", keyReg, defaultReg)
        moveResult(defaultReg)
        ifEqz(defaultReg, "stitch_setting_disabled")
        const4(nullReg, 0)
        returnObject(nullReg)
        label("stitch_setting_disabled")
    }
}

fun Method.returnTrueWhen(setting: ToggleSetting) {
    returnBooleanWhen(setting, true)
}

fun Method.returnFalseWhen(setting: ToggleSetting) {
    returnBooleanWhen(setting, false)
}

private fun Method.returnBooleanWhen(setting: ToggleSetting, value: Boolean) {
    require(info.returnType == "Z") {
        "returnBooleanWhen('${setting.key}') requires a boolean method, got ${info.returnType} in " +
            "${info.classDescriptor}->${info.methodName}"
    }

    val scratch = findFreeRegisters(0, 3)
    require(scratch.size >= 3 && scratch.all { it <= 15 }) {
        "Cannot insert boolean return for '${setting.key}' in ${info.classDescriptor}->${info.methodName}: " +
            "no three free 4-bit scratch registers"
    }

    val keyReg = scratch[0]
    val defaultReg = scratch[1]
    val valueReg = scratch[2]
    if (outsSize < 2) setRegisters(registersSize, 2)

    addInstructions(0) {
        constString(keyReg, setting.key)
        const4(defaultReg, if (setting.default) 1 else 0)
        invokeStatic(SETTINGS_CLASS, "getBoolean", "(Ljava/lang/String;Z)Z", keyReg, defaultReg)
        moveResult(defaultReg)
        ifEqz(defaultReg, "stitch_setting_disabled")
        const4(valueReg, if (value) 1 else 0)
        return_(valueReg)
        label("stitch_setting_disabled")
    }
}
