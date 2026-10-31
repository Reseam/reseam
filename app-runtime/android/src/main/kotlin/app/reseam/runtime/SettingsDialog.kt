// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.app.Dialog
import android.os.Build
import android.window.OnBackInvokedDispatcher

internal fun settingsDialog(activity: Activity, back: () -> Unit, closed: () -> Unit): Dialog {
    val dialog =
        object : Dialog(activity, android.R.style.Theme_DeviceDefault_NoActionBar) {
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
