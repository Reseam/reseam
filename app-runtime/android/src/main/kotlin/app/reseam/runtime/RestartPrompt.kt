// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Context
import android.content.Intent

/**
 * Offers a restart after the settings close with a changed value of a setting that takes effect
 * only on restart.
 */
internal fun offerRestart(activity: Activity, bundle: ReseamBundle, opened: Map<String, *>) {
    if (activity.isFinishing || opened == bundle.settings.restartValues()) return
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
