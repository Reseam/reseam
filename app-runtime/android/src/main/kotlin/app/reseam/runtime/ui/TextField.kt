// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.text.InputType
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView

class TextField internal constructor(private val ui: Ui, labelText: CharSequence?) :
    FrameLayout(ui.context) {
    val field = EditText(ui.context)
    val text: String
        get() = this.field.text.toString()

    private val label = ui.text(labelText, ui.type.labelMedium, ui.colors.onSurfaceVariant)
    private val outline =
        FieldOutline(
            ui.colors,
            ui.px(Tokens.RADIUS_XS).toFloat(),
            ui.px(1).toFloat(),
            ui.px(2).toFloat(),
        )
    private var actionView: ImageView? = null

    init {
        val colors = ui.colors
        val labelHeight =
            Math.round(
                TypedValue.applyDimension(
                    TypedValue.COMPLEX_UNIT_SP,
                    ui.type.labelMedium.lineHeight,
                    resources.displayMetrics,
                )
            )
        field.apply {
            ui.theme.style(this, ui.type.bodyLarge)
            setTextColor(colors.onSurface)
            setHintTextColor(colors.onSurfaceVariant)
            minHeight = ui.px(Tokens.SEARCH_BAR)
            gravity = Gravity.CENTER_VERTICAL or Gravity.START
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            setSingleLine(true)
            val padding = ui.px(Tokens.SPACE_LG)
            setPadding(padding, padding, padding, padding)
            background = outline
            setOnFocusChangeListener { _, focused ->
                outline.setFocused(focused)
                label.setTextColor(if (focused) colors.primary else colors.onSurfaceVariant)
            }
        }
        addView(
            field,
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
                topMargin = labelHeight / 2
            },
        )
        label.apply {
            setPadding(ui.px(Tokens.SPACE_XS), 0, ui.px(Tokens.SPACE_XS), 0)
            labelFor = View.generateViewId()
        }
        field.id = label.labelFor
        addView(
            label,
            LayoutParams(
                    LayoutParams.WRAP_CONTENT,
                    LayoutParams.WRAP_CONTENT,
                    Gravity.TOP or Gravity.START,
                )
                .apply {
                    marginStart = ui.px(Tokens.SPACE_MD)
                },
        )
        label.addOnLayoutChangeListener { _, left, _, right, _, _, _, _, _ ->
            outline.setGap(left.toFloat(), right.toFloat())
        }
    }

    fun text(value: CharSequence?): TextField = apply {
        field.setText(value)
        field.setSelection(field.length())
    }

    fun multiline(lines: Int): TextField = apply {
        field.setSingleLine(false)
        field.inputType =
            InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE
        field.minLines = lines
        field.gravity = Gravity.TOP or Gravity.START
    }

    fun action(
        icon: VectorImage,
        description: CharSequence?,
        onClick: View.OnClickListener?,
    ): TextField = apply {
        actionView?.let(::removeView)
        actionView =
            ui.iconButton(icon, ui.colors.onSurfaceVariant, description, onClick).also {
                addView(
                    it,
                    LayoutParams(
                            ui.px(Tokens.TOUCH_TARGET),
                            ui.px(Tokens.TOUCH_TARGET),
                            Gravity.END or Gravity.TOP,
                        )
                        .apply {
                            topMargin =
                                (field.layoutParams as LayoutParams).topMargin +
                                    ui.px(Tokens.SPACE_XS)
                            marginEnd = ui.px(Tokens.SPACE_XS)
                        },
                )
            }
        field.setPaddingRelative(
            field.paddingStart,
            field.paddingTop,
            ui.px(Tokens.TOUCH_TARGET + Tokens.SPACE_XS),
            field.paddingBottom,
        )
    }

    fun showAction(visible: Boolean) {
        actionView?.visibility = if (visible) VISIBLE else INVISIBLE
    }
}
