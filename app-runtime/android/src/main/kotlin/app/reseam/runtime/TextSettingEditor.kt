// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime

import android.widget.Toast
import app.reseam.runtime.ui.Icons
import app.reseam.runtime.ui.Ui
import app.reseam.runtime.ui.afterTextChanged

internal fun editTextSetting(
    ui: Ui,
    bundle: ReseamBundle,
    text: SchemaSetting.Text,
    store: (String) -> Unit,
) =
    editText(
        ui,
        EditedText(
            text.title?.resolve(bundle.name),
            text.summary?.resolve(bundle.name),
            text.defaultValue,
            if (text.multiline) MULTILINE else 1,
        ),
        bundle.settings.getString(text.key, text.defaultValue),
        check = {
            if (RowCode.VALIDATE in text.code) Reseam.validateSetting(bundle.name, text.key, it)
            else null
        },
        store,
    )

internal class EditedText(
    val title: CharSequence?,
    val message: CharSequence?,
    val default: String,
    val lines: Int,
)

internal fun editText(
    ui: Ui,
    edited: EditedText,
    entered: String,
    check: (String) -> CharSequence?,
    store: (String) -> Unit,
) {
    val field = ui.textField(edited.title).text(entered)
    if (edited.lines > 1) field.multiline(edited.lines)
    field.action(Icons.RESET, RuntimeStrings.restoreDefault) { field.text(edited.default) }
    field.showAction(field.text != edited.default)
    field.field.afterTextChanged { field.showAction(it != edited.default) }
    ui.dialog()
        .title(edited.title)
        .message(edited.message)
        .view(field)
        .dismissAction(RuntimeStrings.cancel)
        .action(RuntimeStrings.save) {
            val problem = check(field.text)
            if (problem == null) store(field.text)
            else {
                Toast.makeText(ui.context, problem, Toast.LENGTH_LONG).show()
                editText(ui, edited, field.text, check, store)
            }
        }
        .show()
}

private const val MULTILINE = 3
