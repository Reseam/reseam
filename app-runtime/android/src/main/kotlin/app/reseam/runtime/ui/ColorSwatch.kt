// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Canvas
import android.graphics.Paint
import android.view.View

class ColorSwatch internal constructor(ui: Ui, private val size: Int) : View(ui.context) {
    private val outline = ui.colors.outline
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val stroke = ui.px(1).toFloat()

    var color: Int = 0
        set(value) {
            field = value
            invalidate()
        }

    init {
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        setMeasuredDimension(size, size)
    }

    override fun onDraw(canvas: Canvas) {
        val radius = size / 2f - stroke
        paint.style = Paint.Style.FILL
        paint.color = color
        canvas.drawCircle(size / 2f, size / 2f, radius, paint)
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = stroke
        paint.color = outline
        canvas.drawCircle(size / 2f, size / 2f, radius, paint)
    }
}
