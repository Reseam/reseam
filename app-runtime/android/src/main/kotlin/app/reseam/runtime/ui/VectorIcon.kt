// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.Path
import android.graphics.PixelFormat
import android.graphics.RectF
import android.graphics.drawable.Drawable

/** Size is the intrinsic size in pixels of the image's longer side. */
class VectorIcon(private val image: VectorImage, private val size: Int, private val tint: Int) :
    Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val matrix = Matrix()
    private val scaled = Path()

    override fun draw(canvas: Canvas) {
        matrix.setRectToRect(
            RectF(0f, 0f, image.width, image.height),
            RectF(bounds),
            Matrix.ScaleToFit.CENTER,
        )
        val scale = matrix.mapRadius(1f)
        val alpha = paint.alpha
        image.shapes.forEach { shape ->
            shape.path.transform(matrix, scaled)
            shape.fill?.let {
                paint.style = Paint.Style.FILL
                paint.color = it.resolve(tint)
                paint.alpha = Math.round(paint.alpha * alpha / 255f)
                canvas.drawPath(scaled, paint)
            }
            shape.stroke?.let {
                paint.style = Paint.Style.STROKE
                paint.color = it.ink.resolve(tint)
                paint.alpha = Math.round(paint.alpha * alpha / 255f)
                paint.strokeWidth = it.width * scale
                paint.strokeCap = it.cap
                paint.strokeJoin = it.join
                canvas.drawPath(scaled, paint)
            }
            paint.alpha = alpha
        }
    }

    override fun getIntrinsicWidth(): Int = Math.round(size * minOf(1f, image.width / image.height))

    override fun getIntrinsicHeight(): Int =
        Math.round(size * minOf(1f, image.height / image.width))

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    @Suppress("DEPRECATION") override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
