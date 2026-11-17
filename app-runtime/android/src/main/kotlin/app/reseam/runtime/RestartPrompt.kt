// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Context
import android.content.Intent

/**
 * What takes effect only when the app restarts: the bundle's settings marked so, and Reseam's
 * language, which text already on the app's screens keeps until then.
 */
internal data class RestartState(val settings: Map<String, *>, val language: String?)

internal fun ReseamBundle.restartState() =
    RestartState(settings.restartValues(), Appearance.language)

/** Offers a restart after the settings close with a change that takes effect only on restart. */
internal fun offerRestart(activity: Activity, bundle: ReseamBundle, opened: RestartState) {
    if (activity.isFinishing || opened == bundle.restartState()) return
    bundle
        .ui(activity)
        .dialog()
        .title(RuntimeStrings.restartTitle)
        .message(RuntimeStrings.restartMessage)
        .dismissAction(RuntimeStrings.later)
        .action(RuntimeStrings.restart) { restart(activity) }
        .show()
}

internal fun restart(context: Context) {
    val launch =
        checkNotNull(context.packageManager.getLaunchIntentForPackage(context.packageName)) {
            "The app has no launcher activity to restart into"
        }
    context.startActivity(Intent.makeRestartActivityTask(launch.component))
    Runtime.getRuntime().exit(0)
}
