package dev.stitch.patch.settings

import dev.stitch.patch.CompatiblePackage
import dev.stitch.patch.PatchRuntime
import dev.stitch.patch.StitchPatch

sealed interface StitchSetting<T> {
    val key: String
    val title: String
    val summary: String?
    val default: T
}

data class ToggleSetting(
    override val key: String,
    override val title: String,
    override val summary: String? = null,
    override val default: Boolean,
) : StitchSetting<Boolean>

data class TextSetting(
    override val key: String,
    override val title: String,
    override val summary: String? = null,
    override val default: String,
) : StitchSetting<String>

data class FolderSetting(
    override val key: String,
    override val title: String,
    override val summary: String? = null,
    override val default: String,
) : StitchSetting<String>

data class ChoiceSetting(
    override val key: String,
    override val title: String,
    override val summary: String? = null,
    override val default: String,
    val choices: List<Choice>,
) : StitchSetting<String>

data class Choice(
    val value: String,
    val title: String,
)

data class SettingsSection(
    val title: String,
    val settings: List<StitchSetting<*>>,
)

interface SettingsHost {
    val appId: String
    val extensionDex: List<String>
    val schemaPath: String get() = "assets/stitch/settings.json"

    fun install(ctx: PatchRuntime)
}

fun settingsHostPatch(
    name: String,
    description: String = "",
    compatibleWith: List<CompatiblePackage> = emptyList(),
    enabledByDefault: Boolean = true,
    dependsOn: List<Any> = emptyList(),
    host: SettingsHost,
): StitchPatch {
    val dependencies = dependsOn.map { dep ->
        when (dep) {
            is String -> dep
            is StitchPatch -> dep.name
            else -> error("dependsOn accepts String or StitchPatch, got: ${dep::class}")
        }
    }
    return object : StitchPatch {
        override val name = name
        override val description = description
        override val dependencies = dependencies
        override val compatibleWith = compatibleWith
        override val enabled = enabledByDefault
        override val extensionDex = host.extensionDex

        override fun execute(ctx: PatchRuntime) = Unit

        override fun afterDependents(ctx: PatchRuntime) {
            val schema = StitchSettingsRegistry.schema(hostKey = name, appId = host.appId)
            ctx.files.write(host.schemaPath, schema.toByteArray(Charsets.UTF_8))
            host.install(ctx)
            StitchSettingsRegistry.clear(name)
        }
    }
}

internal fun registerPatchSettings(
    hostKey: String?,
    patchName: String,
    settings: List<SettingsSection>,
) {
    if (hostKey == null || settings.isEmpty()) return
    StitchSettingsRegistry.register(hostKey, patchName, settings)
}

private object StitchSettingsRegistry {
    private val sectionsByHost =
        linkedMapOf<String, LinkedHashMap<String, List<SettingsSection>>>()

    fun register(hostKey: String, patchName: String, sections: List<SettingsSection>) {
        val hostSections = sectionsByHost.getOrPut(hostKey) { linkedMapOf() }
        hostSections[patchName] = sections
    }

    fun clear(hostKey: String) {
        sectionsByHost.remove(hostKey)
    }

    fun schema(hostKey: String, appId: String): String {
        val patchSections = sectionsByHost[hostKey].orEmpty()
        return buildString {
            append("{\"appId\":")
            appendJson(appId)
            append(",\"sections\":[")
            var firstSection = true
            for ((patchName, sections) in patchSections) {
                for (section in sections) {
                    if (!firstSection) append(',')
                    firstSection = false
                    append("{\"patch\":")
                    appendJson(patchName)
                    append(",\"title\":")
                    appendJson(section.title)
                    append(",\"settings\":[")
                    section.settings.forEachIndexed { index, setting ->
                        if (index > 0) append(',')
                        appendSetting(setting)
                    }
                    append("]}")
                }
            }
            append("]}")
        }
    }
}

private fun StringBuilder.appendSetting(setting: StitchSetting<*>) {
    append("{\"type\":")
    appendJson(
        when (setting) {
            is ToggleSetting -> "toggle"
            is TextSetting -> "text"
            is FolderSetting -> "folder"
            is ChoiceSetting -> "choice"
        },
    )
    append(",\"key\":")
    appendJson(setting.key)
    append(",\"title\":")
    appendJson(setting.title)
    append(",\"summary\":")
    if (setting.summary == null) {
        append("null")
    } else {
        appendJson(setting.summary!!)
    }
    append(",\"default\":")
    when (setting) {
        is ToggleSetting -> append(setting.default)
        is TextSetting -> appendJson(setting.default)
        is FolderSetting -> appendJson(setting.default)
        is ChoiceSetting -> appendJson(setting.default)
    }
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
            '\u000C' -> append("\\f")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '\t' -> append("\\t")
            else -> {
                if (ch.code < 0x20) {
                    append("\\u")
                    append(ch.code.toString(16).padStart(4, '0'))
                } else {
                    append(ch)
                }
            }
        }
    }
    append('"')
}
