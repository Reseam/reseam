// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

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
    showValue: Boolean = true,
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
            showValue,
        )
    }

@Deprecated(
    "Bundles built against 1.0 call text without showValue",
    level = DeprecationLevel.HIDDEN,
)
fun text(
    default: String,
    multiline: Boolean = false,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) = text(default, multiline, key, icon, takesEffect, showValue = true)

fun number(
    default: Double,
    slider: Slider,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        NumberSetting(it.key, it.property, it.title, it.summary, default, icon, takesEffect, slider)
    }

fun range(
    default: ClosedFloatingPointRange<Double>,
    slider: Slider,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        RangeSetting(it.key, it.property, it.title, it.summary, default, icon, takesEffect, slider)
    }

/** [default] is ARGB, such as `0xFFFF0033`. */
fun color(
    default: Long,
    opacity: Boolean = false,
    choice: ChoiceSetting? = null,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        require(default in 0..0xFFFF_FFFF) { "${it.key}: $default is not an ARGB color" }
        ColorSetting(
            it.key,
            it.property,
            it.title,
            it.summary,
            default.toInt(),
            icon,
            takesEffect,
            opacity,
            choice,
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

fun file(
    default: String = "",
    mimeTypes: List<String> = listOf("*/*"),
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) =
    SettingDelegate(key) {
        require(mimeTypes.isNotEmpty()) { "A file setting needs at least one MIME type" }
        FileSetting(
            it.key,
            it.property,
            it.title,
            it.summary,
            default,
            mimeTypes,
            icon,
            takesEffect,
        )
    }

fun choice(
    default: String,
    choices: List<Choice>,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
    showValue: Boolean = true,
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
            showValue,
        )
    }

@Deprecated(
    "Bundles built against 1.0 call choice without showValue",
    level = DeprecationLevel.HIDDEN,
)
fun choice(
    default: String,
    choices: List<Choice>,
    key: String? = null,
    icon: Vector? = null,
    takesEffect: TakesEffect = TakesEffect.ON_RESTART,
) = choice(default, choices, key, icon, takesEffect, showValue = true)

/** Its title is optional, for a row that shows only its summary and the view a patch draws. */
fun action(key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) { ActionSetting(it.key, it.property, it.optionalTitle, it.summary, icon) }

/**
 * Its title is optional: a note without one, and without a summary, shows only the view patch code
 * draws with [onView].
 */
fun note(key: String? = null, icon: Vector? = null) =
    SettingDelegate(key) { NoteSetting(it.key, it.property, it.optionalTitle, it.summary, icon) }

/**
 * A note made while patching, for rows that depend on the patch run, such as which patches it
 * applied. It belongs to no group, so [key] must differ from every declared setting's.
 */
fun noteOf(key: String, title: StringRef, summary: StringRef?) =
    NoteSetting(key, null, title, summary, null)

internal class SettingDeclaration(
    val key: String,
    val property: String,
    val optionalTitle: StringRef?,
    val summary: StringRef?,
    private val missingTitle: String,
) {
    val title: StringRef
        get() = optionalTitle ?: error(missingTitle)
}

class SettingDelegate<S : Setting<*>>
internal constructor(
    private val key: String?,
    private val create: (SettingDeclaration) -> S,
) : PropertyDelegateProvider<SettingsGroup, ReadOnlyProperty<SettingsGroup, S>> {
    override fun provideDelegate(
        thisRef: SettingsGroup,
        property: KProperty<*>,
    ): ReadOnlyProperty<SettingsGroup, S> {
        val name = snakeCase(property.name)
        val strings = thisRef.strings
        val setting =
            create(
                SettingDeclaration(
                    key ?: (thisRef.keyPrefix + "." + name),
                    property.name,
                    strings.find(name),
                    strings.find(name + "_summary"),
                    "${thisRef::class.simpleName}.${property.name} has no title: add '$name' to $strings",
                )
            )
        return ReadOnlyProperty { _, _ -> setting }
    }
}
