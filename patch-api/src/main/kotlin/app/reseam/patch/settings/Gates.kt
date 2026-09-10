// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.MethodTarget
import app.reseam.patch.Otherwise
import app.reseam.patch.PointTarget
import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.before

/** Runs `block` in the app when the toggle is on. */
fun CodeScope.whenEnabled(setting: ToggleSetting, block: CodeScope.() -> Unit): Otherwise =
    whenTrue(call(ReseamSettings.getBoolean, string(setting.key), bool(setting.default)), block)

fun MethodTarget.before(gate: ToggleSetting, block: CodeScope.() -> Unit) = before { whenEnabled(gate, block) }
fun MethodTarget.after(gate: ToggleSetting, block: CodeScope.() -> Unit) = after { whenEnabled(gate, block) }
fun PointTarget.before(gate: ToggleSetting, block: CodeScope.() -> Unit) = before { whenEnabled(gate, block) }
fun PointTarget.after(gate: ToggleSetting, block: CodeScope.() -> Unit) = after { whenEnabled(gate, block) }

/** Returns immediately when the toggle is on; the method must be void. */
fun MethodTarget.skipWhen(setting: ToggleSetting) {
    require(returnType == Type.Void) { "skipWhen(${setting.key}) needs a void method, got $returnType in $descriptor" }
    before(setting) { returnVoid() }
}

fun MethodTarget.returnNullWhen(setting: ToggleSetting) {
    require(returnType.startsWith("L") || returnType.startsWith("[")) { "returnNullWhen(${setting.key}) needs an object method, got $returnType in $descriptor" }
    before(setting) { returnNull() }
}

fun MethodTarget.returnTrueWhen(setting: ToggleSetting) = returnBooleanWhen(setting, true)

fun MethodTarget.returnFalseWhen(setting: ToggleSetting) = returnBooleanWhen(setting, false)

private fun MethodTarget.returnBooleanWhen(setting: ToggleSetting, value: Boolean) {
    require(returnType == Type.Boolean) { "return${value}When(${setting.key}) needs a boolean method, got $returnType in $descriptor" }
    before(setting) { if (value) returnTrue() else returnFalse() }
}
