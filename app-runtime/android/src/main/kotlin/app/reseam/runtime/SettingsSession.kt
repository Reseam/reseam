// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.app.Activity
import android.app.Dialog
import android.os.Build
import android.os.LocaleList
import android.view.View
import android.view.WindowInsetsController
import android.widget.FrameLayout
import app.reseam.runtime.ui.Motion
import app.reseam.runtime.ui.Ui

internal class SettingsSession(
    val bundle: ReseamBundle,
    val host: Activity,
    val dialog: Dialog,
    private val frame: FrameLayout,
    private val back: () -> Unit,
    private val open: (String) -> Unit,
) {
    lateinit var ui: Ui
        private set

    var locales: LocaleList = Reseam.context.resources.configuration.locales
        private set

    val pageIds = mutableListOf<String>()
    private var page: View? = null
    private var transition: Animator? = null

    enum class Direction {
        NONE,
        FORWARD,
        BACKWARD,
    }

    fun display(direction: Direction) {
        transition?.end()
        val previous = page
        val next = settingsPage(bundle, ui, pageIds.last(), back, open, ::toggleTheme)
        page = next
        frame.addView(next)
        // Insets reach only views attached when the window first lays out.
        frame.requestApplyInsets()
        if (previous == null) return
        if (direction == Direction.NONE) {
            frame.removeView(previous)
            return
        }
        val animation = Motion.sharedAxisX(previous, next, direction == Direction.FORWARD)
        transition = animation
        animation.apply {
            addListener(
                object : AnimatorListenerAdapter() {
                    override fun onAnimationEnd(animation: Animator) {
                        frame.removeView(previous)
                        transition = null
                    }
                }
            )
            start()
        }
    }

    @Suppress("DEPRECATION")
    fun theme() {
        ui = bundle.ui(dialog.context)
        locales = Reseam.context.resources.configuration.locales
        val window = checkNotNull(dialog.window) { "A dialog always owns a window" }
        val surface = ui.colors.surface
        window.decorView.setBackgroundColor(surface)
        window.statusBarColor = surface
        window.navigationBarColor = surface
        val light = !ui.theme.dark
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val appearance =
                WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
            checkNotNull(window.insetsController) { "The dialog window supports system bars" }
                .setSystemBarsAppearance(
                    if (light) appearance else 0,
                    appearance,
                )
        } else {
            val decor = window.decorView
            val flags =
                View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
            val visibility = decor.systemUiVisibility
            decor.systemUiVisibility =
                if (light) visibility or flags else visibility and flags.inv()
        }
        display(Direction.NONE)
    }

    private fun toggleTheme() {
        Appearance.toggle(dialog.context)
        theme()
    }

    fun release() {
        transition?.end()
        page = null
        pageIds.clear()
    }
}
