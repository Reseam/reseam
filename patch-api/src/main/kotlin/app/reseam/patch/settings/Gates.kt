// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

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

/**
 * Applies the hooks written in [block] only while [setting] is on. Hermes hooks read it once per
 * process through the `settings-js` extension, which the patch bundle provides along with the
 * `ReseamSettings` React Native module its settings host registers.
 *
 * ```kotlin
 * gate(AppSettings.hideAds) {
 *     loadAds.alwaysReturnNull()
 *     isFeatureEnabled.wrap(Features.isFeatureEnabled)
 * }
 * ```
 */
fun gate(setting: ToggleSetting, block: GateScope.() -> Unit) = GateScope(setting).block()

/** Hooks that run only while a [gate]'s setting is on. Each mirrors the ungated hook. */
class GateScope internal constructor(val setting: ToggleSetting) {
    fun MethodTarget.before(block: CodeScope.() -> Unit) = beforeOn(setting, block)

    fun MethodTarget.after(block: CodeScope.() -> Unit) = afterOn(setting, block)

    fun PointTarget.before(block: CodeScope.() -> Unit) = beforeOn(setting, block)

    fun PointTarget.after(block: CodeScope.() -> Unit) = afterOn(setting, block)

    /** Skips the call at the point; the call's result must be unused. */
    fun PointTarget.skip() = skipWhen {
        call(ReseamSettings.getBoolean, string(setting.key), bool(setting.default))
    }

    fun MethodTarget.alwaysReturn() = returnOn(setting, Type.Void) { returnVoid() }

    fun MethodTarget.alwaysReturnNull() {
        require(returnType.startsWith("L") || returnType.startsWith("[")) {
            "the ${setting.key} gate returns null, but $descriptor returns $returnType"
        }
        beforeOn(setting) { returnNull() }
    }

    fun MethodTarget.alwaysReturn(value: Boolean) =
        returnOn(setting, Type.Boolean) { if (value) returnTrue() else returnFalse() }

    fun MethodTarget.alwaysReturn(value: Int) =
        returnOn(setting, Type.Int) { returnValue(int(value)) }

    fun MethodTarget.alwaysReturn(value: Long) =
        returnOn(setting, Type.Long) { returnValue(long(value)) }

    fun MethodTarget.alwaysReturn(value: String) =
        returnOn(setting, Type.String) { returnValue(string(value)) }

    fun FunctionTarget.wrap(export: JsExport) =
        wrapBound(
            ReseamJsSettings.wrapWhen,
            bound() + HermesArgument.Export(HermesExportRef(export.module.name, export.name)),
        )

    fun FunctionTarget.alwaysReturn() = wrapBound(ReseamJsSettings.skipWhen, bound())

    fun FunctionTarget.alwaysReturnNull() = wrapBound(ReseamJsSettings.returnNullWhen, bound())

    fun FunctionTarget.alwaysReturn(value: Boolean) = returnValue(HermesArgument.Bool(value))

    fun FunctionTarget.alwaysReturn(value: Int) = returnValue(HermesArgument.Int(value))

    fun FunctionTarget.alwaysReturn(value: String) = returnValue(HermesArgument.Text(value))

    /**
     * Calls the function with a copy of argument [index] whose property at [path] is [value].
     * [path] names nested properties with dots, such as `options.compact`; each object along it is
     * shallow-copied, so the caller's object is not changed.
     */
    fun FunctionTarget.setArgument(index: Int, path: String, value: Boolean) {
        require(index >= 0) { "setArgument needs a nonnegative argument index, got $index" }
        require(path.split('.').none(String::isEmpty)) {
            "setArgument needs property names separated by dots, got '$path'"
        }
        wrapBound(
            ReseamJsSettings.setArgumentWhen,
            bound() +
                listOf(
                    HermesArgument.Int(index),
                    HermesArgument.Text(path),
                    HermesArgument.Bool(value),
                ),
        )
    }

    private fun FunctionTarget.returnValue(value: HermesArgument) =
        wrapBound(ReseamJsSettings.returnWhen, bound() + value)

    private fun bound() =
        listOf(HermesArgument.Text(setting.key), HermesArgument.Bool(setting.default))
}

private fun MethodTarget.beforeOn(setting: ToggleSetting, block: CodeScope.() -> Unit) = before {
    whenEnabled(setting, block)
}

private fun MethodTarget.afterOn(setting: ToggleSetting, block: CodeScope.() -> Unit) = after {
    whenEnabled(setting, block)
}

private fun PointTarget.beforeOn(setting: ToggleSetting, block: CodeScope.() -> Unit) = before {
    whenEnabled(setting, block)
}

private fun PointTarget.afterOn(setting: ToggleSetting, block: CodeScope.() -> Unit) = after {
    whenEnabled(setting, block)
}

private fun MethodTarget.returnOn(
    setting: ToggleSetting,
    type: String,
    block: CodeScope.() -> Unit,
) {
    require(returnType == type) {
        "the ${setting.key} gate returns $type, but $descriptor returns $returnType"
    }
    beforeOn(setting, block)
}
