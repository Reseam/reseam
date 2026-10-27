// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.app.Application
import android.content.ComponentCallbacks
import android.content.res.Configuration
import android.os.Bundle
import android.util.Log
import android.view.ViewGroup
import android.widget.FrameLayout
import app.reseam.runtime.ui.Theme
import java.lang.ref.WeakReference

/**
 * A full-screen dialog works with the installed manifest even when patches are mounted over an app.
 */
internal class SettingsPanel : Application.ActivityLifecycleCallbacks, ComponentCallbacks {
    private var resumed = WeakReference<Activity>(null)
    private var state: PanelState = PanelState.Hidden()
    private val visible: PanelState.Visible
        get() = checkNotNull(state as? PanelState.Visible) { "Settings are not open" }

    private fun push(pageId: String) {
        val current = visible.session
        current.pageIds.add(pageId)
        current.display(SettingsSession.Direction.FORWARD)
    }

    private fun back() {
        val current = visible.session
        if (current.pageIds.size == 1) {
            current.dialog.dismiss()
            return
        }
        current.pageIds.removeAt(current.pageIds.lastIndex)
        current.display(SettingsSession.Direction.BACKWARD)
    }

    fun show(
        bundle: ReseamBundle,
        ids: List<String>,
        opened: Map<String, *>? = state.restore?.opened,
    ) {
        if (state is PanelState.Visible) return
        val activity = resumed.get()
        if (activity == null) {
            Log.e(LOG_TAG, "Cannot open settings: no screen of the app is in the foreground")
            return
        }
        val before = opened ?: bundle.settings.snapshot()
        val dialog = settingsDialog(activity, ::back, ::closed)
        val frame = FrameLayout(dialog.context)
        dialog.setContentView(frame)
        checkNotNull(dialog.window) { "A dialog always owns a window" }
            .setLayout(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
        val current = SettingsSession(bundle, activity, dialog, frame, ::back, ::push)
        state =
            PanelState.Visible(
                current,
                before,
                state.restore?.copy(bundle = bundle, opened = before),
            )
        current.pageIds.addAll(ids)
        current.theme()
        dialog.show()
    }

    private fun closed() {
        val current = visible
        current.session.release()
        state = PanelState.Hidden(current.restore?.copy(opened = null))
        offerRestart(current.session.host, current.session.bundle, current.opened)
    }

    override fun onConfigurationChanged(configuration: Configuration) {
        val current = (state as? PanelState.Visible)?.session ?: return
        val darkChanged = current.ui.theme.dark != Theme.of(current.bundle.brand).dark
        if (darkChanged || current.locales != Reseam.context.resources.configuration.locales) {
            current.theme()
        }
    }

    override fun onLowMemory() {}

    override fun onActivityResumed(activity: Activity) {
        resumed = WeakReference(activity)
        val restore = state.restore ?: return
        if (activity.javaClass != restore.host) return
        state = (state as? PanelState.Visible)?.copy(restore = null) ?: PanelState.Hidden()
        show(restore.bundle, restore.pages, restore.opened)
    }

    override fun onActivityDestroyed(activity: Activity) {
        val showing = state as? PanelState.Visible ?: return
        val current = showing.session
        if (activity !== current.host) return
        // Dismiss listeners run later, after a recreated host has already resumed.
        current.dialog.setOnDismissListener(null)
        current.dialog.dismiss()
        val restore =
            if (activity.isChangingConfigurations)
                PanelRestore(
                    current.bundle,
                    showing.opened,
                    current.pageIds.toList(),
                    activity.javaClass,
                )
            else showing.restore?.copy(opened = null)
        current.release()
        state = PanelState.Hidden(restore)
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}

    override fun onActivityStarted(activity: Activity) {}

    override fun onActivityPaused(activity: Activity) {}

    override fun onActivityStopped(activity: Activity) {}

    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
}
