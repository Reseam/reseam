// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.FunctionTarget
import app.reseam.patch.JsExport
import app.reseam.patch.MethodTarget
import app.reseam.patch.Otherwise
import app.reseam.patch.PointTarget
import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.before
import app.reseam.patch.skipWhen
import app.reseam.patch.types.HermesArgument
import app.reseam.patch.types.HermesExportRef

/** Runs `block` in the app when the toggle is on. */
fun CodeScope.whenEnabled(setting: ToggleSetting, block: CodeScope.() -> Unit): Otherwise =
    whenTrue(call(ReseamSettings.getBoolean, string(setting.key), bool(setting.default)), block)

fun MethodTarget.before(gate: ToggleSetting, block: CodeScope.() -> Unit) = before {
    whenEnabled(gate, block)
}

fun MethodTarget.after(gate: ToggleSetting, block: CodeScope.() -> Unit) = after {
    whenEnabled(gate, block)
}

fun PointTarget.before(gate: ToggleSetting, block: CodeScope.() -> Unit) = before {
    whenEnabled(gate, block)
}

fun PointTarget.after(gate: ToggleSetting, block: CodeScope.() -> Unit) = after {
    whenEnabled(gate, block)
}

/** Returns immediately when the toggle is on; the method must be void. */
fun MethodTarget.skipWhen(setting: ToggleSetting) {
    require(returnType == Type.Void) {
        "skipWhen(${setting.key}) needs a void method, got $returnType in $descriptor"
    }
    before(setting) { returnVoid() }
}

/** Skips the call at the point when the toggle is on; the call's result must be unused. */
fun PointTarget.skipWhen(setting: ToggleSetting) = skipWhen {
    call(ReseamSettings.getBoolean, string(setting.key), bool(setting.default))
}

fun MethodTarget.returnNullWhen(setting: ToggleSetting) {
    require(returnType.startsWith("L") || returnType.startsWith("[")) {
        "returnNullWhen(${setting.key}) needs an object method, got $returnType in $descriptor"
    }
    before(setting) { returnNull() }
}

fun MethodTarget.returnTrueWhen(setting: ToggleSetting) = returnBooleanWhen(setting, true)

fun MethodTarget.returnFalseWhen(setting: ToggleSetting) = returnBooleanWhen(setting, false)

private fun MethodTarget.returnBooleanWhen(setting: ToggleSetting, value: Boolean) {
    require(returnType == Type.Boolean) {
        "return${value}When(${setting.key}) needs a boolean method, got $returnType in $descriptor"
    }
    before(setting) { if (value) returnTrue() else returnFalse() }
}

/**
 * Hermes gates read the toggle once per process, through the `ReseamSettings` React Native module
 * the app's settings host registers.
 */
/** Returns `undefined` without calling the function when the toggle is on. */
fun FunctionTarget.skipWhen(setting: ToggleSetting) =
    wrap(ReseamJsSettings.skipWhen, setting.bound())

fun FunctionTarget.returnNullWhen(setting: ToggleSetting) =
    wrap(ReseamJsSettings.returnNullWhen, setting.bound())

fun FunctionTarget.returnTrueWhen(setting: ToggleSetting) =
    wrap(ReseamJsSettings.returnTrueWhen, setting.bound())

fun FunctionTarget.returnFalseWhen(setting: ToggleSetting) =
    wrap(ReseamJsSettings.returnFalseWhen, setting.bound())

/** Wraps the function with [export] when the toggle is on, as [FunctionTarget.wrap] does. */
fun FunctionTarget.wrapWhen(setting: ToggleSetting, export: JsExport) =
    wrap(
        ReseamJsSettings.wrapWhen,
        setting.bound() + HermesArgument.Export(HermesExportRef(export.module.name, export.name)),
    )

/**
 * Calls the function with a copy of argument [index] whose property at [path] is [value] when the
 * toggle is on. [path] names nested properties with dots, such as `options.renderReplies`; each
 * object along it is shallow-copied, so the caller's object is not changed.
 */
fun FunctionTarget.setArgumentWhen(
    setting: ToggleSetting,
    index: Int,
    path: String,
    value: Boolean,
) {
    require(index >= 0) { "setArgumentWhen(${setting.key}) needs a nonnegative argument index" }
    require(path.split('.').none(String::isEmpty)) {
        "setArgumentWhen(${setting.key}) needs property names separated by dots, got '$path'"
    }
    wrap(
        ReseamJsSettings.setArgumentWhen,
        setting.bound() +
            listOf(
                HermesArgument.Int(index),
                HermesArgument.Text(path),
                HermesArgument.Bool(value),
            ),
    )
}

private fun ToggleSetting.bound() = listOf(HermesArgument.Text(key), HermesArgument.Bool(default))
