// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Canvas
import android.graphics.Paint
import android.os.Bundle
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction
import android.view.accessibility.AccessibilityNodeInfo.RangeInfo
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * A slider over the whole positions `0..steps`, with one thumb or, for a range, two that cannot
 * pass each other. Dragging reports each position a thumb passes; lifting it, or a key or
 * accessibility step, reports the settled position. Keys and accessibility steps move the thumb
 * touched last.
 */
class SliderView internal constructor(ui: Ui, private val steps: Int, thumbs: Int) :
    View(ui.context) {
    private val colors = ui.colors
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val density = resources.displayMetrics.density
    private var listener: OnChangeListener? = null
    private val touchSlop = ViewConfiguration.get(ui.context).scaledTouchSlop
    private var downX = 0f
    private var downY = 0f
    private var dragging = false
    private val thumbs = IntArray(thumbs)
    private var active = thumbs - 1

    val positions: List<Int>
        get() = thumbs.toList()

    init {
        require(thumbs in 1..2) { "A slider has one thumb, or two for a range" }
        require(steps > 0) { "A slider needs at least one step" }
        minimumHeight = ui.px(Tokens.TOUCH_TARGET)
        isFocusable = true
    }

    fun interface OnChangeListener {
        /** [position] is the moved thumb's; [settled] is false while a drag is still moving it. */
        fun onChange(view: SliderView, position: Int, settled: Boolean)
    }

    fun setOnChangeListener(listener: OnChangeListener?) {
        this.listener = listener
    }

    /**
     * Moves the thumbs, from the start, without notifying the listener, for changes made elsewhere.
     */
    fun setPosition(vararg positions: Int) {
        require(positions.size == thumbs.size) { "Expected ${thumbs.size} positions" }
        positions.forEachIndexed { index, position -> thumbs[index] = position.coerceIn(0, steps) }
        thumbs.sort()
        invalidate()
    }

    private fun move(position: Int, settled: Boolean) {
        val low = if (active > 0) thumbs[active - 1] else 0
        val high = if (active < thumbs.size - 1) thumbs[active + 1] else steps
        val clamped = position.coerceIn(low, high)
        val changed = clamped != thumbs[active]
        thumbs[active] = clamped
        invalidate()
        if (changed || settled) listener?.onChange(this, clamped, settled)
    }

    private fun nearest(position: Int): Int {
        val distances = thumbs.map { abs(it - position) }
        val closest = distances.min()
        return if (thumbs.size == 2 && distances[0] == distances[1])
            if (position < thumbs[0]) 0 else 1
        else distances.indexOf(closest)
    }

    private val trackStart: Float
        get() = paddingLeft + THUMB * density / 2

    private val trackEnd: Float
        get() = width - paddingRight - THUMB * density / 2

    private fun positionAt(x: Float): Int {
        val fraction = ((x - trackStart) / (trackEnd - trackStart)).coerceIn(0f, 1f)
        val directed = if (layoutDirection == LAYOUT_DIRECTION_RTL) 1f - fraction else fraction
        return (directed * steps).roundToInt()
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!isEnabled) return false
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downX = event.x
                downY = event.y
                dragging = false
                active = nearest(positionAt(event.x))
            }
            MotionEvent.ACTION_MOVE -> {
                val dx = abs(event.x - downX)
                val dy = abs(event.y - downY)
                if (!dragging && dy > touchSlop && dy > dx) return false
                if (!dragging && dx > touchSlop) {
                    dragging = true
                    parent?.requestDisallowInterceptTouchEvent(true)
                }
                if (dragging) move(positionAt(event.x), settled = false)
            }
            MotionEvent.ACTION_UP -> move(positionAt(event.x), settled = true)
            MotionEvent.ACTION_CANCEL -> if (dragging) move(thumbs[active], settled = true)
        }
        return true
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val forward = if (layoutDirection == LAYOUT_DIRECTION_RTL) -1 else 1
        val delta =
            when (keyCode) {
                KeyEvent.KEYCODE_DPAD_RIGHT -> forward
                KeyEvent.KEYCODE_DPAD_LEFT -> -forward
                else -> return super.onKeyDown(keyCode, event)
            }
        if (isEnabled) move(thumbs[active] + delta, settled = true)
        return true
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        setMeasuredDimension(
            getDefaultSize(suggestedMinimumWidth, widthMeasureSpec),
            resolveSize(suggestedMinimumHeight, heightMeasureSpec),
        )
    }

    private fun xOf(position: Int): Float {
        val fraction = position.toFloat() / steps
        val directed = if (layoutDirection == LAYOUT_DIRECTION_RTL) 1f - fraction else fraction
        return trackStart + (trackEnd - trackStart) * directed
    }

    override fun onDraw(canvas: Canvas) {
        val center = height / 2f
        paint.strokeCap = Paint.Cap.ROUND
        paint.strokeWidth = TRACK * density
        paint.color = colors.surfaceContainerHighest
        canvas.drawLine(trackStart, center, trackEnd, center, paint)
        paint.color = colors.primary
        val activeStart = if (thumbs.size == 2) thumbs[0] else 0
        canvas.drawLine(xOf(activeStart), center, xOf(thumbs.last()), center, paint)
        thumbs.forEach { canvas.drawCircle(xOf(it), center, THUMB * density / 2, paint) }
    }

    override fun getAccessibilityClassName(): CharSequence = android.widget.SeekBar::class.java.name

    override fun onInitializeAccessibilityNodeInfo(info: AccessibilityNodeInfo) {
        super.onInitializeAccessibilityNodeInfo(info)
        val position = thumbs[active]
        info.rangeInfo =
            RangeInfo.obtain(RangeInfo.RANGE_TYPE_INT, 0f, steps.toFloat(), position.toFloat())
        if (!isEnabled) return
        if (position > 0) info.addAction(AccessibilityAction.ACTION_SCROLL_BACKWARD)
        if (position < steps) info.addAction(AccessibilityAction.ACTION_SCROLL_FORWARD)
    }

    override fun performAccessibilityAction(action: Int, arguments: Bundle?): Boolean {
        val delta =
            when (action) {
                AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> 1
                AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> -1
                else -> return super.performAccessibilityAction(action, arguments)
            }
        if (!isEnabled) return false
        move(thumbs[active] + delta, settled = true)
        return true
    }
}

private const val TRACK = 4
private const val THUMB = 20
