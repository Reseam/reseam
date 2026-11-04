// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.RunList
import app.reseam.patch.ValueRef
import app.reseam.patch.before

/**
 * Code [onAction] adds: a [CodeScope] with the activity the settings are shown over as [activity].
 */
interface ActionScope : CodeScope {
    val activity: ValueRef
}

/**
 * Runs `block` in the app each time the user taps the running bundle's [action] on its settings
 * screen. Every action a settings screen shows needs code from a patch of the same run; the
 * settings host fails without it.
 *
 * ```kotlin
 * settings(appSettings, section(AppStrings.sectionAccount, AppSettings.exportLogin))
 * execute {
 *     onAction(AppSettings.exportLogin) { Login.copy(activity) }
 * }
 * ```
 */
fun onAction(action: ActionSetting, block: ActionScope.() -> Unit) {
    Reseam.runAction.target.before {
        whenTrue(param(0).callVirtual(STRING, "equals", EQUALS, string(bundle.name))) {
            whenTrue(param(1).callVirtual(STRING, "equals", EQUALS, string(action.key))) {
                ActionCode(this, param(2)).block()
                returnVoid()
            }
        }
    }
    coded.add(action)
}

/** Fails for an action in [sections] that no patch of this run gave code with [onAction]. */
internal fun requireActionCode(sections: List<SettingsSection>) {
    val missing =
        sections
            .flatMap { it.settings }
            .filterIsInstance<ActionSetting>()
            .filter { action -> coded.values.none { it === action } }
    require(missing.isEmpty()) {
        "The settings show ${missing.joinToString { it.key }}, which no patch gives code with onAction"
    }
}

private val coded = RunList<ActionSetting>()

private const val STRING = "java.lang.String"
private const val EQUALS = "(Ljava/lang/Object;)Z"

private class ActionCode(code: CodeScope, override val activity: ValueRef) :
    ActionScope, CodeScope by code {
    override val thisObject: ValueRef
        get() = error("onAction code runs in a static hook; use activity")
}
