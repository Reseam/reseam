// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.animation.ArgbEvaluator
import android.animation.ValueAnimator
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.widget.Checkable

class SwitchView internal constructor(ui: Ui) : View(ui.context), Checkable {
    private val colors = ui.colors
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val track = RectF()
    private val evaluator = ArgbEvaluator()
    private val density = resources.displayMetrics.density
    private var checked = false
    // Zero is off, one is on; intermediate values are animation frames.
    private var progress = 0f
    private var animator: ValueAnimator? = null
    private var listener: OnCheckedChangeListener? = null

    init {
        minimumWidth = ui.px(TRACK_WIDTH)
        minimumHeight = ui.px(TRACK_HEIGHT)
        setOnClickListener { toggle() }
    }

    fun interface OnCheckedChangeListener {
        fun onCheckedChanged(view: SwitchView, checked: Boolean)
    }

    fun setOnCheckedChangeListener(listener: OnCheckedChangeListener?) {
        this.listener = listener
    }

    override fun isChecked(): Boolean = checked

    /** Updates the value without notifying the listener, for changes made elsewhere. */
    override fun setChecked(checked: Boolean) {
        if (this.checked == checked) return
        this.checked = checked
        animateTo(if (checked) 1f else 0f)
    }

    override fun toggle() {
        setChecked(!checked)
        listener?.onCheckedChanged(this, checked)
    }

    private fun animateTo(target: Float) {
        animator?.cancel()
        if (!isLaidOut) {
            progress = target
            invalidate()
            return
        }
        animator =
            ValueAnimator.ofFloat(progress, target).apply {
                duration = DURATION
                addUpdateListener {
                    progress = it.animatedValue as Float
                    invalidate()
                }
                start()
            }
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        setMeasuredDimension(
            resolveSize(suggestedMinimumWidth, widthMeasureSpec),
            resolveSize(suggestedMinimumHeight, heightMeasureSpec),
        )
    }

    override fun onDraw(canvas: Canvas) {
        val width = TRACK_WIDTH * density
        val height = TRACK_HEIGHT * density
        val left = (this.width - width) / 2
        val top = (this.height - height) / 2
        val radius = height / 2
        track.set(left, top, left + width, top + height)
        paint.style = Paint.Style.FILL
        paint.color = blend(colors.surfaceContainerHighest, colors.primary)
        canvas.drawRoundRect(track, radius, radius, paint)
        if (progress < 1f) {
            val inset = OUTLINE * density / 2
            track.inset(inset, inset)
            paint.style = Paint.Style.STROKE
            paint.strokeWidth = OUTLINE * density
            paint.color = Colors.withOpacity(colors.outline, 1f - progress)
            canvas.drawRoundRect(track, radius - inset, radius - inset, paint)
        }
        val thumb = (THUMB_OFF + (THUMB_ON - THUMB_OFF) * progress) * density / 2
        val start = left + radius
        val end = left + width - radius
        paint.style = Paint.Style.FILL
        paint.color = blend(colors.outline, colors.onPrimary)
        canvas.drawCircle(start + (end - start) * progress, top + radius, thumb, paint)
    }

    private fun blend(off: Int, on: Int): Int = evaluator.evaluate(progress, off, on) as Int

    override fun setEnabled(enabled: Boolean) {
        super.setEnabled(enabled)
        alpha = if (enabled) 1f else Tokens.DISABLED
    }

    override fun getAccessibilityClassName(): CharSequence = android.widget.Switch::class.java.name

    override fun onInitializeAccessibilityNodeInfo(info: AccessibilityNodeInfo) {
        super.onInitializeAccessibilityNodeInfo(info)
        info.isCheckable = true
        info.isChecked = checked
    }

    override fun onInitializeAccessibilityEvent(event: AccessibilityEvent) {
        super.onInitializeAccessibilityEvent(event)
        event.isChecked = checked
    }
}

private const val TRACK_WIDTH = 52
private const val TRACK_HEIGHT = 32
private const val OUTLINE = 2
private const val THUMB_ON = 24
private const val THUMB_OFF = 16
private const val DURATION = 150L
