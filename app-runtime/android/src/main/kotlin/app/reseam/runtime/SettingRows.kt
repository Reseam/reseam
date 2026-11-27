// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.view.View
import app.reseam.runtime.ui.ListItem
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.activity

/** Keeps rows on pages and in search synchronized with values changed in either place. */
internal class SettingRows(val bundle: ReseamBundle, val ui: Ui) : Settings.OnChangeListener {
    private val refreshes = mutableMapOf<String, MutableList<() -> Unit>>()

    // Rows whose state patch code computes may depend on any setting.
    private val everyChange = mutableListOf<() -> Unit>()
    val settings = bundle.settings
    private val refreshComputed = { everyChange.forEach { it() } }

    fun follow(screen: View) {
        screen.addOnAttachStateChangeListener(
            object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    settings.addOnChangeListener(this@SettingRows)
                    bundle.screens.add(refreshComputed)
                }

                override fun onViewDetachedFromWindow(v: View) {
                    settings.removeOnChangeListener(this@SettingRows)
                    bundle.screens.remove(refreshComputed)
                }
            }
        )
    }

    override fun onChanged(key: String?) {
        refreshes[key]?.forEach { it() }
        everyChange.forEach { it() }
    }

    fun create(setting: SchemaSetting): ListItem {
        val item = ui.listItem(setting.title?.resolve(bundle.name)).icon(setting.icon)
        when (setting) {
            is SchemaSetting.Toggle -> toggle(item, setting)
            is SchemaSetting.Choice -> choice(item, setting)
            is SchemaSetting.Folder -> folder(item, setting)
            is SchemaSetting.File -> file(item, setting)
            is SchemaSetting.Text -> text(item, setting)
            is SchemaSetting.Number -> number(item, setting)
            is SchemaSetting.Range -> range(item, setting)
            is SchemaSetting.Color -> color(item, setting)
            is SchemaSetting.Action -> action(item, setting)
            is SchemaSetting.Note -> item.supporting(summary(setting))
        }
        if (RowCode.VIEW in setting.code) {
            Reseam.settingView(bundle.name, setting.key, ui.context)?.let { view ->
                item.below(view)
                onAnyChange { view.invalidate() }
            }
        }
        if (RowCode.SUMMARY in setting.code) {
            onAnyChange {
                Reseam.settingSummary(bundle.name, setting.key, settings.text(setting), ui.context)
                    ?.let(item::supporting)
            }
        }
        val refresh = { item.isEnabled = editable(setting) }
        val coverers =
            if (setting is SchemaSetting.Toggle) settings.coverers(setting.key) else emptyList()
        (listOfNotNull(setting.enabledBy) + coverers).forEach { onChange(it, refresh) }
        if (RowCode.ENABLED in setting.code) onAnyChange(refresh) else refresh()
        return item
    }

    fun onChange(key: String, refresh: () -> Unit) {
        refresh()
        refreshes.getOrPut(key) { mutableListOf() }.add(refresh)
    }

    private fun onAnyChange(refresh: () -> Unit) {
        refresh()
        everyChange.add(refresh)
    }

    private fun editable(setting: SchemaSetting): Boolean =
        !(setting is SchemaSetting.Toggle && settings.isCovered(setting.key)) &&
            setting.enabledBy?.let { settings.getBoolean(it, false) } != false &&
            (RowCode.ENABLED !in setting.code ||
                Reseam.settingEnabled(bundle.name, setting.key, ui.context))

    fun store(setting: SchemaSetting, store: () -> Unit) {
        val previous = settings.text(setting)
        store()
        changed(setting, previous)
    }

    private fun changed(setting: SchemaSetting, previous: String) {
        if (RowCode.CHANGE in setting.code && settings.text(setting) != previous) {
            Reseam.settingChanged(bundle.name, setting.key, previous, activity(ui.context))
        }
    }

    fun summary(setting: SchemaSetting): CharSequence? = setting.summary?.resolve(bundle.name)

    fun chosen(choice: SchemaSetting.Choice): SchemaSetting.Choice.Option =
        choice.options[choice.index(settings.getString(choice.key, choice.defaultValue))]

    private fun toggle(item: ListItem, toggle: SchemaSetting.Toggle) {
        val view = ui.switchView()
        view.setOnCheckedChangeListener { _, checked ->
            store(toggle) { settings.setBoolean(toggle.key, checked) }
        }
        item.trailing(view).onClick { view.toggle() }
        val showValue: () -> Unit = {
            val on = settings.getBoolean(toggle.key, toggle.defaultValue)
            view.setChecked(on)
            item.supporting(toggle.summary(on)?.resolve(bundle.name))
        }
        (listOf(toggle.key) + settings.coverers(toggle.key)).forEach { onChange(it, showValue) }
    }

    private fun choice(item: ListItem, choice: SchemaSetting.Choice) {
        onChange(choice.key) {
            item.supporting(
                if (choice.showValue) chosen(choice).title.resolve(bundle.name) else summary(choice)
            )
        }
        item.onClick {
            ui.dialog()
                .title(choice.title?.resolve(bundle.name))
                .message(summary(choice))
                .choices(
                    choice.options.map { it.title.resolve(bundle.name) }.toTypedArray(),
                    choice.options.indexOf(chosen(choice)),
                ) { index ->
                    store(choice) { settings.setString(choice.key, choice.options[index].value) }
                }
                .dismissAction(RuntimeStrings.cancel)
                .show()
        }
    }

    private fun text(item: ListItem, text: SchemaSetting.Text) {
        if (text.showValue) item.textSummary(text) { value, show -> show(value) }
        else item.supporting(summary(text))
        item.onClick {
            editTextSetting(ui, bundle, text) { value ->
                store(text) { settings.setString(text.key, value) }
            }
        }
    }

    private fun action(item: ListItem, action: SchemaSetting.Action) {
        item.supporting(summary(action)).onClick {
            Reseam.runAction(bundle.name, action.key, activity(ui.context))
        }
    }

    private fun folder(item: ListItem, folder: SchemaSetting.Folder) {
        item.textSummary(folder) { value, show -> show(folderName(value)) }
        item.onClick {
            val previous = settings.text(folder)
            pickFolder(activity(ui.context), bundle.name, folder.key) { changed(folder, previous) }
        }
    }

    private fun file(item: ListItem, file: SchemaSetting.File) {
        item.textSummary(file) { value, show -> fileName(ui.context, value, show) }
        item.onClick {
            val previous = settings.text(file)
            pickFile(activity(ui.context), bundle.name, file.key, file.mimeTypes) {
                changed(file, previous)
            }
        }
    }

    private fun ListItem.textSummary(
        setting: SchemaSetting.Textual,
        display: (value: String, show: (String) -> Unit) -> Unit,
    ) =
        onChange(setting.key) {
            val value = settings.getString(setting.key, setting.defaultValue)
            if (value.isEmpty()) supporting(summary(setting))
            else
                display(value) { shown ->
                    if (settings.getString(setting.key, setting.defaultValue) == value) {
                        supporting(shown)
                    }
                }
        }
}
