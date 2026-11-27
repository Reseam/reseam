// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import app.reseam.patch.CodeScope
import app.reseam.patch.RunList
import app.reseam.patch.ValueRef

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
    Reseam.runAction.forSetting(action, { returnVoid() }) { ActionCode(this, param(2)).block() }
    coded.add(action)
}

internal fun requireActionCode(contributions: List<SettingsContribution>) {
    val missing =
        contributions
            .filterIsInstance<SettingsSection>()
            .flatMap { it.settings }
            .filterIsInstance<ActionSetting>()
            .filter { action -> coded.values.none { it === action } }
    require(missing.isEmpty()) {
        "The settings show ${missing.joinToString { it.key }}, which no patch gives code with onAction"
    }
}

private val coded = RunList<ActionSetting>()

private class ActionCode(code: CodeScope, override val activity: ValueRef) :
    StaticHookCode(code, "onAction code runs in a static hook; use activity"), ActionScope
