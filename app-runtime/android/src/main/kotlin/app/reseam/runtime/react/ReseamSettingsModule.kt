// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.react

import app.reseam.runtime.Reseam
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod

class ReseamSettingsModule internal constructor(context: ReactApplicationContext) :
    ReactContextBaseJavaModule(context) {
    override fun getName(): String = SETTINGS_MODULE_NAME

    @ReactMethod(isBlockingSynchronousMethod = true)
    fun isEnabled(bundle: String, key: String, defaultValue: Boolean): Boolean =
        Reseam.isEnabled(bundle, key, defaultValue)

    @ReactMethod
    fun openSettings(bundle: String) {
        Reseam.openSettings(bundle)
    }
}

internal const val SETTINGS_MODULE_NAME = "ReseamSettings"
