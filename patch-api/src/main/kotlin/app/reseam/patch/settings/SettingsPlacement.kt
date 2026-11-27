// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

internal class SettingsPlacement(
    contributions: List<SettingsContribution>,
    layout: List<SettingsContribution>,
) {
    val sections = placed(contributions.filterIsInstance<SettingsSection>(), layout)
    val stored =
        storedOnly(contributions, layout, sections) +
            sections
                .flatMap { it.settings }
                .filterIsInstance<ColorSetting>()
                .mapNotNull { it.choice }
}

/**
 * One row per shown key, as the last patch to show it defined it: a dependent runs after the
 * patches it depends on, so its copy of their setting wins. The [layout] places the keys it lists.
 * The rest stay where the first patch to show them put them, in sections merged by page and
 * heading.
 */
private fun placed(
    contributions: List<SettingsSection>,
    layout: List<SettingsContribution>,
): List<SettingsSection> {
    requireConsistent(layout)
    val shown = contributions.flatMap { it.settings }.associateBy { it.key }
    val laidOut =
        layout.filterIsInstance<SettingsSection>().map { section ->
            section.copy(settings = section.settings.mapNotNull { shown[it.key] })
        }
    val placedKeys = layout.flatMap { it.settings }.mapTo(mutableSetOf()) { it.key }
    val rest =
        contributions
            .groupBy { it.page?.id to it.title }
            .map { (_, merged) ->
                merged
                    .first()
                    .copy(
                        settings =
                            merged
                                .flatMap { it.settings }
                                .filter { placedKeys.add(it.key) }
                                .map { shown.getValue(it.key) }
                    )
            }
    return (laidOut + rest).filter { it.settings.isNotEmpty() }
}

private fun requireConsistent(layout: List<SettingsContribution>) {
    val twice =
        layout
            .flatMap { part ->
                part.settings.map { ((part as? SettingsSection)?.page?.id) to it.key }
            }
            .groupBy { it }
            .filterValues { it.size > 1 }
            .keys
            .map { it.second }
    require(twice.isEmpty()) { "The settings layout lists $twice twice on one page" }
    val shownKeys =
        layout
            .filterIsInstance<SettingsSection>()
            .flatMap { it.settings }
            .mapTo(hashSetOf()) { it.key }
    val both =
        layout
            .filterIsInstance<StoredSettings>()
            .flatMap { it.settings }
            .map { it.key }
            .filter { it in shownKeys }
    require(both.isEmpty()) { "The settings layout both shows and stores $both" }
}

private fun storedOnly(
    contributions: List<SettingsContribution>,
    layout: List<SettingsContribution>,
    shown: List<SettingsSection>,
): List<Setting<*>> {
    val shownKeys = shown.flatMap { it.settings }.mapTo(hashSetOf()) { it.key }
    val hidden =
        layout
            .filterIsInstance<StoredSettings>()
            .flatMap { it.settings }
            .mapTo(hashSetOf()) { it.key }
    val stored =
        contributions.filterIsInstance<StoredSettings>().flatMap { it.settings } +
            contributions
                .filterIsInstance<SettingsSection>()
                .flatMap { it.settings }
                .filter { it.key in hidden }
    val last = stored.associateBy { it.key }
    return stored.map { it.key }.distinct().filter { it !in shownKeys }.map { last.getValue(it) }
}
