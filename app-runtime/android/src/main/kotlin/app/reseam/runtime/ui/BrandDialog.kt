// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.app.Dialog
import android.graphics.drawable.ColorDrawable
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.ScrollView

/** A Material 3 basic dialog in a bundle's brand. Build one through Ui.dialog(). */
class BrandDialog private constructor() {
    fun interface OnChooseListener {
        fun onChoose(index: Int)
    }

    class Builder internal constructor(private val ui: Ui) {
        private var icon: VectorImage? = null
        private var title: CharSequence? = null
        private var message: CharSequence? = null
        private var view: View? = null
        private var choices = Choices(null, -1, null)
        private val actions = mutableListOf<Action>()
        private var onDismiss: Runnable? = null

        /** An icon above a centered title. */
        fun icon(icon: VectorImage?): Builder = apply { this.icon = icon }

        fun title(title: CharSequence?): Builder = apply { this.title = title }

        fun message(message: CharSequence?): Builder = apply { this.message = message }

        fun view(view: View?): Builder = apply { this.view = view }

        /** Marks the selected choice; choosing one closes the dialog. */
        fun choices(
            choices: Array<out CharSequence?>?,
            selected: Int,
            onChoose: OnChooseListener?,
        ): Builder = apply {
            this.choices = Choices(choices, selected, onChoose)
        }

        /** Closes the dialog before running onClick; the last button is the confirming action. */
        fun action(label: CharSequence?, onClick: Runnable?): Builder = apply {
            actions.add(Action(label, onClick))
        }

        fun dismissAction(label: CharSequence?): Builder = action(label, null)

        fun onDismiss(onDismiss: Runnable?): Builder = apply { this.onDismiss = onDismiss }

        fun show(): Dialog = build().also { it.show() }

        private fun build(): Dialog =
            Dialog(ui.context).apply {
                val padding = ui.px(Tokens.SPACE_XXL)
                requestWindowFeature(Window.FEATURE_NO_TITLE)
                val container =
                    LinearLayout(ui.context).apply {
                        orientation = LinearLayout.VERTICAL
                        background = ui.shape(ui.colors.surfaceContainerHigh, Tokens.RADIUS_XL)
                        setPadding(0, padding, 0, padding)
                    }
                container.addView(dialogHeader(ui, icon, title, message, padding))
                val body = body(this, padding)
                if (body.childCount > 0) {
                    container.addView(
                        ScrollView(ui.context).apply { addView(body) },
                        LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f),
                    )
                }
                if (actions.isNotEmpty()) container.addView(buttons(this, padding))
                setContentView(container)
                onDismiss?.let { callback -> setOnDismissListener { callback.run() } }
                checkNotNull(window) { "A dialog always owns a window" }
                    .apply {
                        setBackgroundDrawable(ColorDrawable(0))
                        addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                        setDimAmount(Tokens.SCRIM)
                        val width =
                            (ui.context.resources.displayMetrics.widthPixels -
                                    2 * ui.px(Tokens.SPACE_XXXL + Tokens.SPACE_SM))
                                .coerceAtMost(ui.px(Tokens.DIALOG_MAX_WIDTH))
                                .coerceAtLeast(ui.px(Tokens.DIALOG_MIN_WIDTH))
                        setLayout(width, ViewGroup.LayoutParams.WRAP_CONTENT)
                    }
            }

        private fun body(dialog: Dialog, padding: Int): LinearLayout =
            LinearLayout(ui.context).apply {
                orientation = LinearLayout.VERTICAL
                choices.titles?.let { titles ->
                    setPadding(0, ui.px(Tokens.SPACE_LG), 0, 0)
                    titles.forEachIndexed { index, title ->
                        addView(
                            ChoiceRow(ui, title, index == choices.selected).apply {
                                setOnClickListener {
                                    dialog.dismiss()
                                    checkNotNull(choices.onChoose) {
                                            "A choice needs a listener when selected"
                                        }
                                        .onChoose(index)
                                }
                            }
                        )
                    }
                }
                view?.let {
                    setPadding(padding, ui.px(Tokens.SPACE_LG), padding, 0)
                    addView(
                        it,
                        LinearLayout.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT,
                        ),
                    )
                }
            }

        private fun buttons(dialog: Dialog, padding: Int): LinearLayout =
            LinearLayout(ui.context).apply {
                gravity = Gravity.END or Gravity.CENTER_VERTICAL
                setPadding(padding, padding, padding - ui.px(Tokens.SPACE_MD), 0)
                actions.forEach { action ->
                    addView(
                        ui.button(action.label) {
                            dialog.dismiss()
                            action.onClick?.run()
                        }
                    )
                }
            }

        private data class Action(val label: CharSequence?, val onClick: Runnable?)

        private data class Choices(
            val titles: Array<out CharSequence?>?,
            val selected: Int,
            val onChoose: OnChooseListener?,
        )
    }
}
