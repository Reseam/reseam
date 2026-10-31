// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.net.Uri
import android.view.View
import app.reseam.runtime.ui.ListItem
import app.reseam.runtime.ui.Ui

/** Keeps rows on pages and in search synchronized with values changed in either place. */
internal class SettingRows(private val bundle: ReseamBundle, private val ui: Ui) :
    Settings.OnChangeListener {
    private val refreshes = mutableMapOf<String, MutableList<() -> Unit>>()
    private val settings = bundle.settings

    fun follow(screen: View) {
        screen.addOnAttachStateChangeListener(
            object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    settings.addOnChangeListener(this@SettingRows)
                }

                override fun onViewDetachedFromWindow(v: View) {
                    settings.removeOnChangeListener(this@SettingRows)
                }
            }
        )
    }

    override fun onChanged(key: String?) {
        refreshes[key]?.forEach { it() }
    }

    fun create(setting: SettingsSchema.Setting): ListItem {
        val item = ui.listItem(setting.title.resolve(bundle.name)).icon(setting.icon)
        when (setting) {
            is SettingsSchema.Toggle -> toggle(item, setting)
            is SettingsSchema.Choice -> choice(item, setting)
            is SettingsSchema.Folder -> folder(item, setting)
            is SettingsSchema.Text -> text(item, setting)
        }
        return item
    }

    private fun onChange(key: String, refresh: () -> Unit) {
        refresh()
        refreshes.getOrPut(key) { mutableListOf() }.add(refresh)
    }

    private fun toggle(item: ListItem, toggle: SettingsSchema.Toggle) {
        val view = ui.switchView()
        view.setOnCheckedChangeListener { _, checked -> settings.setBoolean(toggle.key, checked) }
        item.supporting(toggle.summary?.resolve(bundle.name)).trailing(view).onClick {
            view.toggle()
        }
        val refresh = {
            val covered = settings.isCovered(toggle.key)
            view.setChecked(settings.getBoolean(toggle.key, toggle.defaultValue))
            item.isEnabled = !covered
            // The row already shows the disabled state, so the switch only stops taking taps.
            view.isClickable = !covered
        }
        onChange(toggle.key, refresh)
        settings.coverers(toggle.key).forEach { onChange(it, refresh) }
    }

    private fun choice(item: ListItem, choice: SettingsSchema.Choice) {
        onChange(choice.key) {
            item.supporting(choice.options[selected(choice)].title.resolve(bundle.name))
        }
        item.onClick {
            ui.dialog()
                .title(choice.title.resolve(bundle.name))
                .message(choice.summary?.resolve(bundle.name))
                .choices(
                    choice.options.map { it.title.resolve(bundle.name) }.toTypedArray(),
                    selected(choice),
                ) { index ->
                    settings.setString(choice.key, choice.options[index].value)
                }
                .dismissAction(RuntimeStrings.cancel)
                .show()
        }
    }

    private fun selected(choice: SettingsSchema.Choice): Int =
        choice.index(settings.getString(choice.key, choice.defaultValue))

    private fun text(item: ListItem, text: SettingsSchema.Text) {
        item.stringSummary(text, text.defaultValue)
        item.onClick { editTextSetting(ui, bundle, text) }
    }

    private fun folder(item: ListItem, folder: SettingsSchema.Folder) {
        item.stringSummary(folder, folder.defaultValue, ::displayFolder)
        item.onClick { pickFolder(activity(ui.context), bundle.name, folder.key) }
    }

    private fun ListItem.stringSummary(
        setting: SettingsSchema.Setting,
        defaultValue: String,
        display: (String) -> String = { it },
    ) =
        onChange(setting.key) {
            val value = settings.getString(setting.key, defaultValue)
            supporting(
                if (value.isEmpty()) setting.summary?.resolve(bundle.name) else display(value)
            )
        }
}

/** Only the folder's name is recognizable to a user in a document tree URI. */
private fun displayFolder(value: String): String {
    if (!value.startsWith("content://")) return value
    val last = Uri.parse(value).lastPathSegment ?: return value
    return last.substringAfterLast(':').ifEmpty { value }
}

internal tailrec fun activity(context: Context): Activity =
    when (context) {
        is Activity -> context
        is ContextWrapper -> activity(context.baseContext)
        else -> error("The settings are not shown over an activity")
    }
