// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.RectF
import android.graphics.drawable.Drawable

internal class FieldOutline(
    private val colors: Colors,
    private val radius: Float,
    private val thin: Float,
    private val thick: Float,
) : Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val outlineBounds = RectF()
    private var focused = false
    private var gapStart = 0f
    private var gapEnd = 0f

    fun setFocused(focused: Boolean) {
        this.focused = focused
        invalidateSelf()
    }

    fun setGap(start: Float, end: Float) {
        gapStart = start
        gapEnd = end
        invalidateSelf()
    }

    override fun draw(canvas: Canvas) {
        val width = if (focused) thick else thin
        paint.strokeWidth = width
        paint.color = if (focused) colors.primary else colors.outline
        outlineBounds.set(bounds)
        outlineBounds.inset(width / 2, width / 2)
        canvas.save()
        canvas.clipOutRect(gapStart, outlineBounds.top - width, gapEnd, outlineBounds.top + width)
        canvas.drawRoundRect(outlineBounds, radius, radius, paint)
        canvas.restore()
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
    }

    @Suppress("DEPRECATION") override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
