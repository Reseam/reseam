// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch.settings

import app.reseam.patch.ActiveRuntime
import app.reseam.patch.CompatiblePackage
import app.reseam.patch.ExtClass
import app.reseam.patch.ExtJsModule
import app.reseam.patch.PatchDeclaration
import app.reseam.patch.PatchRuntime
import app.reseam.patch.ReseamPatch
import app.reseam.patch.Type
import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty

/** The runtime class patched apps read settings through; linked from the bundle on first use. */
object ReseamSettings : ExtClass("app.reseam.runtime.settings.ReseamSettings") {
    val getBoolean = static("getBoolean", Type.String, Type.Boolean, returns = Type.Boolean)
    val getString = static("getString", Type.String, Type.String, returns = Type.String)
}

/**
 * The JavaScript the Hermes gates call. It reads [ReseamSettings] through the app's
 * `ReseamSettings` React Native module, which the app's settings host registers.
 */
internal object ReseamJsSettings : ExtJsModule("settings-js") {
    val skipWhen = export("skipWhen")
    val returnNullWhen = export("returnNullWhen")
    val returnTrueWhen = export("returnTrueWhen")
    val returnFalseWhen = export("returnFalseWhen")
    val wrapWhen = export("wrapWhen")
}

const val SETTINGS_SCHEMA_PATH = "assets/reseam/settings.json"

/**
 * A user-facing setting stored in the patched app, shown by the settings screen the host installs.
 */
sealed class Setting<T>(val key: String, val title: String, val summary: String?, val default: T) {
    override fun toString() = key
}

class ToggleSetting(key: String, title: String, summary: String? = null, default: Boolean) :
    Setting<Boolean>(key, title, summary, default)

/** [multiline] lets the value hold one entry per line. */
class TextSetting(
    key: String,
    title: String,
    summary: String? = null,
    default: String,
    val multiline: Boolean = false,
) : Setting<String>(key, title, summary, default)

class FolderSetting(key: String, title: String, summary: String? = null, default: String) :
    Setting<String>(key, title, summary, default)

class ChoiceSetting(
    key: String,
    title: String,
    summary: String? = null,
    default: String,
    val choices: List<Choice>,
) : Setting<String>(key, title, summary, default) {
    init {
        require(choices.any { it.value == default }) {
            "Choice setting '$key' defaults to '$default', which is not a choice"
        }
    }

    /** The same stored setting with other choices, for a patch that extends it from its options. */
    fun copy(default: String = this.default, choices: List<Choice> = this.choices) =
        ChoiceSetting(key, title, summary, default, choices)
}

data class Choice(val value: String, val title: String)

/** A reusable destination. Parents allow arbitrary nesting; lower orders appear first. */
data class SettingsPage(
    val id: String,
    val title: String,
    val parent: SettingsPage? = null,
    val order: Int = 0,
) {
    init {
        require(id.isNotBlank()) { "A settings page needs a non-blank id" }
        require(title.isNotBlank()) { "A settings page needs a non-blank title" }
        var ancestor = parent
        while (ancestor != null) {
            require(ancestor.id != id) { "Settings page '$id' cannot be its own ancestor" }
            ancestor = ancestor.parent
        }
    }
}

data class SettingsSection(
    val title: String,
    val settings: List<Setting<*>>,
    val page: SettingsPage? = null,
)

/** A section on the root settings screen. */
fun section(title: String, vararg settings: Setting<*>) = SettingsSection(title, settings.toList())

/** Contributes settings to a page shared by any number of patches. */
fun section(page: SettingsPage, title: String, vararg settings: Setting<*>) =
    SettingsSection(title, settings.toList(), page)

/**
 * Declares a setting as a property: `val hideAds by toggle("Hide ads", default = true)`. The key
 * derives from the owner and property names unless given.
 */
fun toggle(title: String, summary: String? = null, default: Boolean, key: String? = null) =
    SettingDelegate(key) { ToggleSetting(it, title, summary, default) }

fun text(
    title: String,
    summary: String? = null,
    default: String,
    multiline: Boolean = false,
    key: String? = null,
) = SettingDelegate(key) { TextSetting(it, title, summary, default, multiline) }

fun folder(title: String, summary: String? = null, default: String, key: String? = null) =
    SettingDelegate(key) { FolderSetting(it, title, summary, default) }

fun choice(
    title: String,
    summary: String? = null,
    default: String,
    choices: List<Choice>,
    key: String? = null,
) = SettingDelegate(key) { ChoiceSetting(it, title, summary, default, choices) }

class SettingDelegate<S : Setting<*>>
internal constructor(
    private val key: String?,
    private val create: (key: String) -> S,
) : PropertyDelegateProvider<Any?, ReadOnlyProperty<Any?, S>> {
    override fun provideDelegate(
        thisRef: Any?,
        property: kotlin.reflect.KProperty<*>,
    ): ReadOnlyProperty<Any?, S> {
        val owner = thisRef?.let { it::class.simpleName }?.let { snakeCase(it) + "." }.orEmpty()
        val setting = create(key ?: (owner + snakeCase(property.name)))
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

/**
 * The internal patch that installs the settings runtime and screen for one app. Patches register
 * their sections with it; after they have all run it writes the schema the runtime reads and runs
 * the app-specific install.
 */
class SettingsHost
internal constructor(
    val appId: String,
    override val compatibleWith: List<CompatiblePackage>,
    override val dependencies: List<ReseamPatch>,
    private val ownSections: List<SettingsSection>,
    private val install: PatchRuntime.() -> Unit,
) : ReseamPatch {
    override val name: String? = null
    override val description = "Settings for $appId"

    fun register(patch: ReseamPatch, sections: List<SettingsSection>) {
        if (sections.isEmpty()) return
        ActiveRuntime.current.afterSuccess {
            ActiveRuntime.current.run.register(this, patch, sections)
        }
    }

    /** The host runs before dependents, so its own sections lead the schema. */
    override fun execute(ctx: PatchRuntime) {
        ctx.run.clear(this)
        register(this, ownSections)
    }

    override fun afterDependents(ctx: PatchRuntime) {
        try {
            ctx.files.write(SETTINGS_SCHEMA_PATH, schema(ctx).toByteArray(Charsets.UTF_8))
            ctx.install()
        } finally {
            ctx.run.clear(this)
        }
    }

    private fun schema(ctx: PatchRuntime): String {
        val sections = ctx.run.sections(this).filter { it.settings.isNotEmpty() }
        val pages = linkedMapOf<String, SettingsPage>()
        fun addPage(page: SettingsPage) {
            page.parent?.let(::addPage)
            val previous = pages.putIfAbsent(page.id, page)
            require(previous == null || previous == page) {
                "Conflicting settings page '${page.id}'"
            }
        }
        sections.forEach { it.page?.let(::addPage) }

        return buildString {
            append("{\"appId\":")
            appendJson(appId)
            append(",\"pages\":[")
            pages.values
                .sortedBy { it.order }
                .forEachIndexed { index, page ->
                    if (index > 0) append(',')
                    append("{\"id\":")
                    appendJson(page.id)
                    append(",\"title\":")
                    appendJson(page.title)
                    append(",\"parent\":")
                    page.parent?.let { appendJson(it.id) } ?: append("null")
                    append('}')
                }
            append("],\"sections\":[")
            sections
                .groupBy { it.page?.id to it.title }
                .entries
                .forEachIndexed { index, (location, contributions) ->
                    if (index > 0) append(',')
                    append("{\"page\":")
                    location.first?.let(::appendJson) ?: append("null")
                    append(",\"title\":")
                    appendJson(location.second)
                    append(",\"settings\":[")
                    contributions
                        .flatMap { it.settings }
                        .distinctBy { it.key }
                        .forEachIndexed { settingIndex, setting ->
                            if (settingIndex > 0) append(',')
                            appendSetting(setting)
                        }
                    append("]}")
                }
            append("]}")
        }
    }

    override fun toString() = "$appId settings"
}

class SettingsHostBuilder internal constructor() : PatchDeclaration() {
    private var installBlock: PatchRuntime.() -> Unit = {}
    private val sections = mutableListOf<SettingsSection>()

    /** Sections the host owns, for settings that belong to no single feature patch. */
    fun settings(vararg sections: SettingsSection) {
        this.sections += sections
    }

    /**
     * Wires the settings screen into the app; runs once every patch has registered its sections.
     */
    fun install(block: PatchRuntime.() -> Unit) {
        installBlock = block
    }

    internal fun build(appId: String) =
        SettingsHost(
            appId,
            compatibility.toList(),
            dependencies.toList(),
            sections.toList(),
            installBlock,
        )
}

fun settingsHost(appId: String, block: SettingsHostBuilder.() -> Unit): SettingsHost =
    SettingsHostBuilder().apply(block).build(appId)

private fun StringBuilder.appendSetting(setting: Setting<*>) {
    append("{\"type\":")
    appendJson(
        when (setting) {
            is ToggleSetting -> "toggle"
            is TextSetting -> "text"
            is FolderSetting -> "folder"
            is ChoiceSetting -> "choice"
        }
    )
    append(",\"key\":")
    appendJson(setting.key)
    append(",\"title\":")
    appendJson(setting.title)
    append(",\"summary\":")
    setting.summary?.let(::appendJson) ?: append("null")
    append(",\"default\":")
    when (setting) {
        is ToggleSetting -> append(setting.default)
        is TextSetting -> appendJson(setting.default)
        is FolderSetting -> appendJson(setting.default)
        is ChoiceSetting -> appendJson(setting.default)
    }
    if (setting is TextSetting && setting.multiline) append(",\"multiline\":true")
    if (setting is ChoiceSetting) {
        append(",\"choices\":[")
        setting.choices.forEachIndexed { index, choice ->
            if (index > 0) append(',')
            append("{\"value\":")
            appendJson(choice.value)
            append(",\"title\":")
            appendJson(choice.title)
            append("}")
        }
        append("]")
    }
    append("}")
}

private fun StringBuilder.appendJson(value: String) {
    append('"')
    for (ch in value) {
        when (ch) {
            '\\' -> append("\\\\")
            '"' -> append("\\\"")
            '\b' -> append("\\b")
            '' -> append("\\f")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '\t' -> append("\\t")
            else ->
                if (ch.code < 0x20) append("\\u" + ch.code.toString(16).padStart(4, '0'))
                else append(ch)
        }
    }
    append('"')
}
