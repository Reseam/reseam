// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Application
import android.content.Context
import android.content.Intent
import java.util.concurrent.ConcurrentHashMap

/** Runtime injected into patched apps; bundles share its application context and settings. */
object Reseam {
    private val bundles = ConcurrentHashMap<String, ReseamBundle>()
    @Volatile private var app: Context? = null

    internal val context: Context
        get() = app ?: error("Reseam.init has not run")

    /** Called first in Application.onCreate. Later calls do nothing. */
    @Export
    @JvmStatic
    @Synchronized
    fun init(context: Context) {
        if (app != null) return
        val application = context.applicationContext ?: context
        app = application
        if (application is Application) trackSettings(application)
    }

    /** Until initialization, settings reads return their defaults. */
    @get:JvmStatic
    val isStarted: Boolean
        get() = app != null

    /** Fails before initialization or when the app has no readable settings for this bundle. */
    @JvmStatic
    fun bundle(name: String): ReseamBundle {
        val application = context
        return bundles.computeIfAbsent(name) { ReseamBundle(application, it) }
    }

    @Export
    @JvmStatic
    fun isEnabled(bundle: String, key: String, defaultValue: Boolean): Boolean =
        if (app == null) defaultValue else bundle(bundle).settings.getBoolean(key, defaultValue)

    @Export
    @JvmStatic
    fun getString(bundle: String, key: String, defaultValue: String): String =
        if (app == null) defaultValue else bundle(bundle).settings.getString(key, defaultValue)

    @Export
    @JvmStatic
    fun getChoice(bundle: String, key: String, defaultValue: String): String =
        if (app == null) defaultValue else bundle(bundle).settings.getChoice(key)

    @Export @JvmStatic fun openSettings(bundle: String) = bundle(bundle).openSettings()

    /**
     * Opens bundle settings for the SDK's settings intent, otherwise starts the intent unchanged.
     */
    @Export
    @JvmStatic
    fun startActivity(context: Context, intent: Intent) {
        val data = intent.data
        if (intent.action == OPEN_ACTION && data?.scheme == SCHEME) {
            openSettings(data.schemeSpecificPart)
        } else {
            context.startActivity(intent)
        }
    }
}

private const val OPEN_ACTION = "app.reseam.settings.OPEN"
private const val SCHEME = "reseam-settings"

internal const val LOG_TAG = "ReseamSettings"
