// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.math.tan

/** Receives a cubic Bézier segment: two control points, then the end point. */
internal typealias CubicSink =
    (x1: Double, y1: Double, x2: Double, y2: Double, endX: Double, endY: Double) -> Unit

internal fun quadratic(
    qx: Double,
    qy: Double,
    endX: Double,
    endY: Double,
    fromX: Double,
    fromY: Double,
    cubic: CubicSink,
) =
    cubic(
        fromX + 2.0 / 3 * (qx - fromX),
        fromY + 2.0 / 3 * (qy - fromY),
        endX + 2.0 / 3 * (qx - endX),
        endY + 2.0 / 3 * (qy - endY),
        endX,
        endY,
    )

/** Approximates an elliptical arc with cubics, following SVG's arc implementation notes. */
internal fun arc(
    fromX: Double,
    fromY: Double,
    radiusX: Double,
    radiusY: Double,
    rotationDegrees: Double,
    large: Boolean,
    sweep: Boolean,
    endX: Double,
    endY: Double,
    cubic: CubicSink,
) {
    if (fromX == endX && fromY == endY) return
    var rx = abs(radiusX)
    var ry = abs(radiusY)
    if (rx == 0.0 || ry == 0.0) {
        cubic(fromX, fromY, endX, endY, endX, endY)
        return
    }
    val phi = Math.toRadians(rotationDegrees)
    val cosPhi = cos(phi)
    val sinPhi = sin(phi)
    val halfX = (fromX - endX) / 2
    val halfY = (fromY - endY) / 2
    val x1 = cosPhi * halfX + sinPhi * halfY
    val y1 = -sinPhi * halfX + cosPhi * halfY
    val lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
    if (lambda > 1) {
        rx *= sqrt(lambda)
        ry *= sqrt(lambda)
    }
    val numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
    val denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1
    val root = sqrt((numerator / denominator).coerceAtLeast(0.0)) * if (large == sweep) -1 else 1
    val cx1 = root * rx * y1 / ry
    val cy1 = -root * ry * x1 / rx
    val cx = cosPhi * cx1 - sinPhi * cy1 + (fromX + endX) / 2
    val cy = sinPhi * cx1 + cosPhi * cy1 + (fromY + endY) / 2
    val start = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry)
    var delta = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry)
    if (!sweep && delta > 0) delta -= 2 * Math.PI
    if (sweep && delta < 0) delta += 2 * Math.PI
    val segments = ceil(abs(delta) / (Math.PI / 2)).toInt()
    val step = delta / segments
    val k = 4.0 / 3 * tan(step / 4)
    fun point(
        theta: Double,
        scale: Double = 1.0,
        tangent: Boolean = false,
    ): Pair<Double, Double> {
        val ex = if (tangent) -rx * sin(theta) else rx * cos(theta)
        val ey = if (tangent) ry * cos(theta) else ry * sin(theta)
        return Pair(
            cosPhi * ex * scale - sinPhi * ey * scale,
            sinPhi * ex * scale + cosPhi * ey * scale,
        )
    }
    var theta = start
    repeat(segments) {
        val next = theta + step
        val (sx, sy) = point(theta)
        val (ex, ey) = point(next)
        val (t1x, t1y) = point(theta, k, tangent = true)
        val (t2x, t2y) = point(next, k, tangent = true)
        cubic(
            cx + sx + t1x,
            cy + sy + t1y,
            cx + ex - t2x,
            cy + ey - t2y,
            cx + ex,
            cy + ey,
        )
        theta = next
    }
}

private fun angle(ux: Double, uy: Double, vx: Double, vy: Double): Double =
    atan2(ux * vy - uy * vx, ux * vx + uy * vy)
