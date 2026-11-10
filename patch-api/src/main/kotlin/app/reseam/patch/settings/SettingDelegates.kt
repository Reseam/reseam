// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty

/**
 * Declares a setting as a property of a [SettingsGroup]: `val hideAds by toggle(default = true)`.
 * The key derives from the group and property names unless given.
 */
fun toggle(
    default: Boolean,
    key: String? = null,
    icon: Vector? = null,
    covers: List<ToggleSetting> = emptyList(),
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        ToggleSetting(it.key, it.property, it.title, it.summary, default, icon, takesEffect, covers)
    }

fun text(
    default: String,
    multiline: Boolean = false,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        TextSetting(
            it.key,
            it.property,
            it.title,
            it.summary,
            default,
            multiline,
            icon,
            takesEffect,
        )
    }

fun folder(
    default: String,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        FolderSetting(it.key, it.property, it.title, it.summary, default, icon, takesEffect)
    }

fun choice(
    default: String,
    choices: List<Choice>,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        ChoiceSetting(
            it.key,
            it.property,
            it.title,
            it.summary,
            default,
            choices,
            icon,
            takesEffect,
        )
    }

fun action(key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) { ActionSetting(it.key, it.property, it.title, it.summary, icon) }

internal class SettingDeclaration(
    val key: String,
    val property: String,
    val title: StringRef,
    val summary: StringRef?,
)

class SettingDelegate<S : Setting<*>>
internal constructor(
    private val key: String?,
    private val create: (SettingDeclaration) -> S,
) : PropertyDelegateProvider<SettingsGroup, ReadOnlyProperty<SettingsGroup, S>> {
    override fun provideDelegate(
        thisRef: SettingsGroup,
        property: kotlin.reflect.KProperty<*>,
    ): ReadOnlyProperty<SettingsGroup, S> {
        val name = snakeCase(property.name)
        val strings = thisRef.strings
        val title =
            strings.find(name)
                ?: error(
                    "${thisRef::class.simpleName}.${property.name} has no title: add '$name' to $strings"
                )
        val setting =
            create(
                SettingDeclaration(
                    key ?: (thisRef.keyPrefix + "." + name),
                    property.name,
                    title,
                    strings.find(name + "_summary"),
                )
            )
        return ReadOnlyProperty { _, _ -> setting }
    }
}

internal fun snakeCase(name: String): String = buildString {
    for ((i, ch) in name.withIndex()) {
        if (
            ch.isUpperCase() &&
                i > 0 &&
                (name[i - 1].isLowerCase() || name.getOrNull(i + 1)?.isLowerCase() == true)
        )
            append('_')
        append(ch.lowercaseChar())
    }
}
