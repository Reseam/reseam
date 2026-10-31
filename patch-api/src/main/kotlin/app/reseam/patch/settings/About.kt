// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/** Links on the settings screen's about card: [links] as icon buttons, [actions] as buttons. */
class About(val links: List<Link> = emptyList(), val actions: List<Link> = emptyList())

/** An icon button reads [label] aloud. */
class Link(val label: DisplayText, val url: String, val icon: Vector) {
    init {
        require(url.startsWith("https://")) { "a link opens an https:// page, not '$url'" }
    }
}
