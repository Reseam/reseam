// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.react

import app.reseam.runtime.Export
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext

/**
 * The runtime's React Native package. It names no React Native package type, since apps built with
 * R8 rename them; the patch that adds it makes it implement the app's package interface.
 */
object ReseamReactPackage {
    /** Adds the package once, however many bundles call this. */
    @Export
    @JvmStatic
    fun addTo(packages: MutableList<Any?>) {
        if (packages.none { it === this }) packages.add(this)
    }

    fun createNativeModules(context: ReactApplicationContext): List<NativeModule> =
        listOf(ReseamSettingsModule(context))

    fun createViewManagers(context: ReactApplicationContext): List<Any> = emptyList()
}
