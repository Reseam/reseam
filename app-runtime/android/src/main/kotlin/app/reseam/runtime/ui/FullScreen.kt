// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.app.Activity
import android.app.Dialog
import android.content.Context
import android.content.ContextWrapper
import android.os.Build
import android.view.View
import android.view.Window
import android.view.WindowInsetsController
import android.window.OnBackInvokedDispatcher

internal fun fullScreenDialog(activity: Activity, back: () -> Unit, closed: () -> Unit): Dialog {
    val dialog =
        object :
            Dialog(activity.withRtlSupport(), android.R.style.Theme_DeviceDefault_NoActionBar) {
            override fun onBackPressed() {
                back()
            }
        }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        // Hosts opting into predictive Back never call onBackPressed.
        dialog.onBackInvokedDispatcher.registerOnBackInvokedCallback(
            OnBackInvokedDispatcher.PRIORITY_DEFAULT,
            back,
        )
    }
    dialog.setOnDismissListener { closed() }
    return dialog
}

@Suppress("DEPRECATION")
internal fun Window.paintSurface(ui: Ui) {
    followRuntimeText()
    val surface = ui.colors.surface
    decorView.setBackgroundColor(surface)
    statusBarColor = surface
    navigationBarColor = surface
    val light = !ui.theme.dark
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        val appearance =
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
        checkNotNull(insetsController) { "The dialog window supports system bars" }
            .setSystemBarsAppearance(
                if (light) appearance else 0,
                appearance,
            )
    } else {
        val decor = decorView
        val flags = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
        val visibility = decor.systemUiVisibility
        decor.systemUiVisibility = if (light) visibility or flags else visibility and flags.inv()
    }
}

internal tailrec fun activity(context: Context): Activity =
    when (context) {
        is Activity -> context
        is ContextWrapper -> activity(context.baseContext)
        else -> error("$context belongs to no activity")
    }
