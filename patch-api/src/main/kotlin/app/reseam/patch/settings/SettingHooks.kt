// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.Method
import app.reseam.patch.before
import app.reseam.patch.returnType

private const val SETTINGS_CLASS = "Lapp/reseam/runtime/settings/ReseamSettings;"

fun Method.prependWhen(
    setting: ToggleSetting,
    block: CodeScope.() -> Unit,
) {
    before {
        val enabled = staticCall(
            SETTINGS_CLASS,
            "getBoolean",
            "(Ljava/lang/String;Z)Z",
            string(setting.key),
            int(if (setting.default) 1 else 0),
        )
        ifTrue(enabled) {
            block()
        }
    }
}

fun Method.skipWhen(setting: ToggleSetting) {
    require(returnType == "V") {
        "skipWhen('${setting.key}') requires a void method, got $returnType in " +
            "${info.classDescriptor}->${info.methodName}"
    }
    prependWhen(setting) {
        returnVoid()
    }
}

fun Method.returnNullWhen(setting: ToggleSetting) {
    require(returnType.startsWith("L") || returnType.startsWith("[")) {
        "returnNullWhen('${setting.key}') requires an object/array method, got $returnType in " +
            "${info.classDescriptor}->${info.methodName}"
    }
    prependWhen(setting) {
        returnObject(nullObject())
    }
}

fun Method.returnTrueWhen(setting: ToggleSetting) {
    returnBooleanWhen(setting, true)
}

fun Method.returnFalseWhen(setting: ToggleSetting) {
    returnBooleanWhen(setting, false)
}

private fun Method.returnBooleanWhen(setting: ToggleSetting, value: Boolean) {
    require(returnType == "Z") {
        "returnBooleanWhen('${setting.key}') requires a boolean method, got $returnType in " +
            "${info.classDescriptor}->${info.methodName}"
    }

    prependWhen(setting) {
        if (value) {
            returnTrue()
        } else {
            returnFalse()
        }
    }
}
