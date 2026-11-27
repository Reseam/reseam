// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.ExtMethod
import app.reseam.patch.RunList
import app.reseam.patch.ValueRef
import app.reseam.patch.before

/**
 * Code [onSummary] adds: [value] is the setting's current value as text, and [context] the settings
 * screen.
 */
interface SummaryScope : CodeScope {
    val context: ValueRef
    val value: ValueRef
}

/** Code [onView] adds, with the settings screen as [context]. */
interface ViewScope : CodeScope {
    val context: ValueRef
}

/** Code [onEnabled] adds, with the settings screen as [context]. */
interface EnabledScope : CodeScope {
    val context: ValueRef
}

/** Code [onChange] adds: [previous] is the setting's value before the change, as text. */
interface ChangeScope : ActionScope {
    val previous: ValueRef
}

/** Code [onValidate] adds: [value] is the text the user entered, not yet stored. */
interface ValidateScope : CodeScope {
    val value: ValueRef
}

/**
 * Computes the line under [setting]'s title in the app when its row shows and after any setting
 * changes. `block` returns the text with `returnValue`, a `CharSequence`. Returning null, or not
 * returning, shows the declared summary.
 *
 * ```kotlin
 * onSummary(AppSettings.cacheSize) { returnValue(Cache.describe(context, value)) }
 * ```
 */
fun onSummary(setting: Setting<*>, block: SummaryScope.() -> Unit) =
    rowCode(RowCode.SUMMARY, Reseam.settingSummary, setting, { returnNull() }) {
        SummaryCode(this, context = param(3), value = param(2)).block()
    }

/**
 * Draws a view under [setting]'s title and summary, such as a preview of what the settings around
 * it do. `block` returns an `android.view.View` with `returnValue`, made with [context]. The row
 * redraws it after any setting changes. A row whose title string is empty shows only the view.
 *
 * ```kotlin
 * onView(AppSettings.zonePreview) { returnValue(ZonePreview.create(context)) }
 * ```
 */
fun onView(setting: Setting<*>, block: ViewScope.() -> Unit) =
    rowCode(RowCode.VIEW, Reseam.settingView, setting, { returnNull() }) {
        ViewCode(this, context = param(2)).block()
    }

/**
 * Decides in the app whether [setting]'s row takes changes, for conditions [enabledWhile] cannot
 * express, such as several toggles or the Android version. `block` returns a boolean with
 * `returnValue`, `returnTrue` or `returnFalse`. Not returning leaves the row enabled. The app asks
 * again after any setting changes. With [enabledWhile] too, the row needs both.
 */
fun onEnabled(setting: Setting<*>, block: EnabledScope.() -> Unit) =
    rowCode(RowCode.ENABLED, Reseam.settingEnabled, setting, { returnTrue() }) {
        EnabledCode(this, context = param(2)).block()
    }

/**
 * Checks the text the user entered for [setting] before it is stored. `block` returns an error
 * message with `returnValue`, a `CharSequence`, to keep the old value and show the message.
 * Returning null, or not returning, stores the text.
 */
fun onValidate(setting: TextSetting, block: ValidateScope.() -> Unit) =
    rowCode(RowCode.VALIDATE, Reseam.validateSetting, setting, { returnNull() }) {
        ValidateCode(this, value = param(2)).block()
    }

/**
 * Runs `block` in the app after the user changes [setting] on the settings screen, with the
 * screen's activity as `activity` and the value before the change as `previous`. Changes made by
 * code, or by importing settings, do not run it.
 */
fun onChange(setting: Setting<*>, block: ChangeScope.() -> Unit) =
    rowCode(RowCode.CHANGE, Reseam.settingChanged, setting, { returnVoid() }) {
        ChangeCode(this, activity = param(3), previous = param(2)).block()
    }

/**
 * Lets [setting]'s row take changes only while [toggle] is on, as a sub-setting of it. The row
 * stays visible, greyed out, while the toggle is off.
 */
fun enabledWhile(setting: Setting<*>, toggle: ToggleSetting) =
    enablers.add(setting.key to toggle.key)

internal class RowWiring(val enabler: (String) -> String?, val codes: (String) -> List<RowCode>) {
    companion object {
        val OF_RUN =
            RowWiring(
                { key -> enablers.values.lastOrNull { it.first == key }?.second },
                { key -> rowCodes.values.filter { it.second == key }.map { it.first }.distinct() },
            )

        val NONE = RowWiring({ null }, { emptyList() })
    }
}

internal enum class RowCode(val tag: String) {
    SUMMARY("summary"),
    ENABLED("enabled"),
    VALIDATE("validate"),
    CHANGE("change"),
    VIEW("view"),
}

private val enablers = RunList<Pair<String, String>>()
private val rowCodes = RunList<Pair<RowCode, String>>()

private fun rowCode(
    kind: RowCode,
    hook: ExtMethod,
    setting: Setting<*>,
    otherwise: CodeScope.() -> Unit,
    code: CodeScope.() -> Unit,
) {
    hook.forSetting(setting, otherwise, code)
    rowCodes.add(kind to setting.key)
}

internal fun ExtMethod.forSetting(
    setting: Setting<*>,
    otherwise: CodeScope.() -> Unit,
    code: CodeScope.() -> Unit,
) = target.before {
    whenTrue(param(0).callVirtual(STRING, "equals", EQUALS, string(bundle.name))) {
        whenTrue(param(1).callVirtual(STRING, "equals", EQUALS, string(setting.key))) {
            code()
            otherwise()
        }
    }
}

private const val STRING = "java.lang.String"
private const val EQUALS = "(Ljava/lang/Object;)Z"

internal abstract class StaticHookCode(code: CodeScope, private val noThis: String) :
    CodeScope by code {
    override val thisObject: ValueRef
        get() = error(noThis)
}

private class SummaryCode(
    code: CodeScope,
    override val context: ValueRef,
    override val value: ValueRef,
) : StaticHookCode(code, "onSummary code runs in a static hook; use context"), SummaryScope

private class EnabledCode(code: CodeScope, override val context: ValueRef) :
    StaticHookCode(code, "onEnabled code runs in a static hook; use context"), EnabledScope

private class ViewCode(code: CodeScope, override val context: ValueRef) :
    StaticHookCode(code, "onView code runs in a static hook; use context"), ViewScope

private class ValidateCode(code: CodeScope, override val value: ValueRef) :
    StaticHookCode(code, "onValidate code runs in a static hook"), ValidateScope

private class ChangeCode(
    code: CodeScope,
    override val activity: ValueRef,
    override val previous: ValueRef,
) : StaticHookCode(code, "onChange code runs in a static hook; use activity"), ChangeScope
