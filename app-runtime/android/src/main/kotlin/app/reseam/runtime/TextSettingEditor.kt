// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import app.reseam.runtime.ui.Icons
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.afterTextChanged

internal fun editTextSetting(ui: Ui, bundle: ReseamBundle, text: SettingsSchema.Text) {
    val settings = bundle.settings
    val field =
        ui.textField(text.title.resolve(bundle.name))
            .text(settings.getString(text.key, text.defaultValue))
    if (text.multiline) field.multiline(3)
    field.action(Icons.RESET, RuntimeStrings.restoreDefault) {
        field.text(text.defaultValue)
    }
    field.showAction(field.text != text.defaultValue)
    field.field.afterTextChanged { field.showAction(it != text.defaultValue) }
    ui.dialog()
        .title(text.title.resolve(bundle.name))
        .message(text.summary?.resolve(bundle.name))
        .view(field)
        .dismissAction(RuntimeStrings.cancel)
        .action(RuntimeStrings.save) { settings.setString(text.key, field.text) }
        .show()
}
