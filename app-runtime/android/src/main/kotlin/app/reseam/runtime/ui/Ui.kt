// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Build
import android.view.Gravity
import android.view.View
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView

/**
 * Builds views in a bundle's brand and theme. Obtain one through Reseam.bundle(name).ui(context).
 */
class Ui(@JvmField val context: Context, @JvmField val theme: Theme) {
    val colors: Colors
        get() = theme.colors

    val type: Typography
        get() = theme.type

    fun px(dp: Float): Int = Tokens.px(context, dp)

    internal fun px(dp: Int): Int = px(dp.toFloat())

    fun text(value: CharSequence?, style: TypeStyle, color: Int): TextView =
        TextView(context).apply {
            text = value
            theme.style(this, style)
            setTextColor(color)
        }

    /** Draws the image at sizeDp, using tint where it has no colors of its own. */
    fun icon(image: VectorImage, sizeDp: Int, tint: Int): ImageView =
        ImageView(context).apply {
            setImageDrawable(VectorIcon(image, px(sizeDp), tint))
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }

    fun iconButton(
        image: VectorImage,
        tint: Int,
        description: CharSequence?,
        onClick: View.OnClickListener?,
    ): ImageView =
        icon(image, Tokens.ICON_MD, tint).apply {
            scaleType = ImageView.ScaleType.CENTER
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            contentDescription = description
            background =
                ripple(
                    null,
                    Tokens.TOUCH_TARGET / 2f,
                    Tokens.TOUCH_TARGET / 2f,
                    colors.onSurfaceVariant,
                )
            setOnClickListener(onClick)
            minimumWidth = px(Tokens.TOUCH_TARGET)
            minimumHeight = px(Tokens.TOUCH_TARGET)
        }

    fun shape(color: Int, radiusDp: Float): GradientDrawable = shape(color, radiusDp, radiusDp)

    internal fun shape(color: Int, radiusDp: Int): GradientDrawable =
        shape(color, radiusDp.toFloat())

    fun shape(color: Int, topDp: Float, bottomDp: Float): GradientDrawable =
        GradientDrawable().apply {
            setColor(color)
            val top = px(topDp).toFloat()
            val bottom = px(bottomDp).toFloat()
            cornerRadii = floatArrayOf(top, top, top, top, bottom, bottom, bottom, bottom)
        }

    fun ripple(content: Drawable?, topDp: Float, bottomDp: Float, stateColor: Int): RippleDrawable =
        RippleDrawable(
            ColorStateList.valueOf(Colors.withOpacity(stateColor, Tokens.PRESSED)),
            content,
            shape(0xFFFFFFFF.toInt(), topDp, bottomDp),
        )

    /** A text button in the primary color. */
    fun button(label: CharSequence?, onClick: View.OnClickListener?): TextView =
        text(label, type.labelLarge, colors.primary).apply {
            gravity = Gravity.CENTER
            minHeight = px(Tokens.BUTTON)
            minimumHeight = px(Tokens.BUTTON)
            val horizontal = px(Tokens.SPACE_MD)
            setPadding(horizontal, 0, horizontal, 0)
            val radius = Tokens.BUTTON / 2f
            background = ripple(null, radius, radius, colors.primary)
            setOnClickListener(onClick)
            isFocusable = true
        }

    /** A filled button in the secondary container color. */
    fun tonalButton(
        label: CharSequence?,
        image: VectorImage,
        onClick: View.OnClickListener?,
    ): View =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            minimumHeight = px(Tokens.BUTTON)
            val horizontal = px(Tokens.SPACE_MD)
            setPadding(horizontal, 0, horizontal, 0)
            val radius = Tokens.BUTTON / 2f
            background =
                ripple(
                    shape(colors.secondaryContainer, radius),
                    radius,
                    radius,
                    colors.onSecondaryContainer,
                )
            addView(
                icon(image, Tokens.ICON_SM, colors.onSecondaryContainer),
                LinearLayout.LayoutParams(px(Tokens.ICON_SM), px(Tokens.ICON_SM)).apply {
                    marginEnd = px(Tokens.SPACE_SM)
                },
            )
            addView(text(label, type.labelLarge, colors.onSecondaryContainer))
            setOnClickListener(onClick)
            isFocusable = true
        }

    fun listItem(title: CharSequence?): ListItem = ListItem(this, title)

    fun itemGroup(): ItemGroup = ItemGroup(this)

    fun sectionLabel(label: CharSequence?): TextView =
        text(label, type.labelLarge, colors.primary).apply {
            setPadding(0, px(Tokens.SPACE_LG), 0, px(Tokens.SPACE_SM))
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) isAccessibilityHeading = true
        }

    fun switchView(): SwitchView = SwitchView(this)

    fun searchBar(hint: CharSequence?): SearchBar = SearchBar(this, hint)

    fun textField(label: CharSequence?): TextField = TextField(this, label)

    fun topBar(title: CharSequence?, back: Runnable): TopBar = TopBar(this, title, back)

    fun dialog(): BrandDialog.Builder = BrandDialog.Builder(this)
}
