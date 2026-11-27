// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.app.Application
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.LocaleList
import android.view.View
import java.util.concurrent.ConcurrentHashMap
import java.util.function.Consumer

/** Runtime injected into patched apps; bundles share its application context and settings. */
object Reseam {
    private val bundles = ConcurrentHashMap<String, ReseamBundle>()
    @Volatile private var app: Context? = null

    internal val context: Context
        get() = app ?: error("Reseam.init has not run")

    /**
     * The user's languages for this app, the per-app language included. The app's resources reorder
     * or narrow their own copy to the languages the app translates, which Reseam's text doesn't
     * depend on.
     */
    internal val locales: LocaleList
        get() = LocaleList.getDefault()

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
        if (app == null) defaultValue else bundle(bundle).settings.getChoice(key, defaultValue)

    @Export @JvmStatic fun openSettings(bundle: String) = bundle(bundle).openSettings()

    /**
     * Runs the code patches gave the action [key] of [bundle], tapped over [activity]. The method
     * is empty here; each patch's `onAction` adds its code to it.
     */
    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER")
    fun runAction(bundle: String, key: String, activity: Activity) {}

    /**
     * Opens the system file picker over [activity] for a file of one of [mimeTypes]. [onPicked]
     * gets the file the user picks, readable while the app runs. It does not run when they cancel.
     */
    @JvmStatic
    fun openFile(activity: Activity, mimeTypes: List<String>, onPicked: Consumer<Uri>) =
        requestDocument(activity, openDocument(mimeTypes), null, onPicked::accept)

    /**
     * Asks the user over [activity] where to save a new file of [mimeType], suggesting [name].
     * [onCreated] gets the new, empty file to write. It does not run when they cancel.
     */
    @JvmStatic
    fun saveFile(activity: Activity, name: String, mimeType: String, onCreated: Consumer<Uri>) =
        requestDocument(
            activity,
            Intent(Intent.ACTION_CREATE_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType(mimeType)
                .putExtra(Intent.EXTRA_TITLE, name),
            null,
            onCreated::accept,
        )

    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER", "FunctionOnlyReturningConstant")
    fun settingSummary(
        bundle: String,
        key: String,
        value: String,
        context: Context,
    ): CharSequence? = null

    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER", "FunctionOnlyReturningConstant")
    fun validateSetting(bundle: String, key: String, value: String): CharSequence? = null

    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER")
    fun settingChanged(bundle: String, key: String, previous: String, activity: Activity) {}

    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER", "FunctionOnlyReturningConstant")
    fun settingView(bundle: String, key: String, context: Context): View? = null

    @Export
    @JvmStatic
    @Suppress("UNUSED_PARAMETER", "FunctionOnlyReturningConstant")
    fun settingEnabled(bundle: String, key: String, context: Context): Boolean = true

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
