// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Application
import android.os.Handler
import android.os.Looper

internal data class PanelRestore(
    val bundle: ReseamBundle,
    val opened: Map<String, *>?,
    val pages: List<OpenPage>,
    val host: Class<*>,
)

internal sealed interface PanelState {
    val restore: PanelRestore?

    data class Hidden(override val restore: PanelRestore? = null) : PanelState

    data class Visible(
        val session: SettingsSession,
        val opened: Map<String, *>,
        override val restore: PanelRestore? = null,
    ) : PanelState
}

private val main = Handler(Looper.getMainLooper())
private var settingsPanel: SettingsPanel? = null

internal fun trackSettings(app: Application) {
    val panel = SettingsPanel()
    settingsPanel = panel
    app.registerActivityLifecycleCallbacks(panel)
    app.registerComponentCallbacks(panel)
}

internal fun openSettingsPanel(bundle: ReseamBundle) {
    val panel = settingsPanel ?: error("Reseam.init has not run")
    main.post { panel.show(bundle, listOf(OpenPage(""))) }
}
