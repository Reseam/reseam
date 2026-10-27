// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity

/**
 * A bundle setting. Reads before initialization return the default; writes require initialization.
 */
sealed class SettingValue<T>(val bundle: String, val key: String, val default: T) {
    abstract var value: T

    fun reset() {
        value = default
    }
}

class ToggleValue(bundle: String, key: String, default: Boolean) :
    SettingValue<Boolean>(bundle, key, default) {
    override var value: Boolean
        get() = Reseam.isEnabled(bundle, key, default)
        set(value) {
            Reseam.bundle(bundle).settings.setBoolean(key, value)
        }
}

open class TextValue(bundle: String, key: String, default: String) :
    SettingValue<String>(bundle, key, default) {
    override var value: String
        get() = Reseam.getString(bundle, key, default)
        set(value) {
            Reseam.bundle(bundle).settings.setString(key, value)
        }
}

class FolderValue(bundle: String, key: String, default: String) : TextValue(bundle, key, default) {
    /**
     * Opens the system folder picker over [activity]. The folder the user picks becomes the value,
     * then [onPicked] runs on the main thread; it does not run when the user cancels.
     */
    fun pick(activity: Activity, onPicked: Runnable) = pickFolder(activity, bundle, key, onPicked)
}

class ChoiceValue(bundle: String, key: String, default: String) :
    SettingValue<String>(bundle, key, default) {
    override var value: String
        get() = Reseam.getChoice(bundle, key, default)
        set(value) {
            Reseam.bundle(bundle).settings.setString(key, value)
        }
}
