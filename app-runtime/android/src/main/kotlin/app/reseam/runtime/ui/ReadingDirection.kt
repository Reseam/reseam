// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.content.Context
import android.content.ContextWrapper
import android.content.pm.ApplicationInfo
import android.view.Window
import app.reseam.runtime.RuntimeStrings

/**
 * A context whose views can lay out right to left. Android keeps every view of a host whose
 * manifest leaves out android:supportsRtl left to right, and Reseam's screens follow their own
 * language.
 */
internal fun Context.withRtlSupport(): Context {
    val info = applicationInfo
    if (info.flags and ApplicationInfo.FLAG_SUPPORTS_RTL != 0) return this
    val supported =
        ApplicationInfo(info).apply { flags = flags or ApplicationInfo.FLAG_SUPPORTS_RTL }
    return object : ContextWrapper(this) {
        override fun getApplicationInfo(): ApplicationInfo = supported
    }
}

/** Lays the window out the way the runtime's own text reads, whatever direction the host uses. */
internal fun Window.followRuntimeText() {
    decorView.layoutDirection = RuntimeStrings.table.layoutDirection
}
