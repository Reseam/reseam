// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

import kotlin.math.floor

/**
 * Rewrites SVG path data as absolute move, line, cubic and close commands, shifted by an offset, so
 * the app runtime needs no SVG parser. Arcs and quadratic curves become cubics.
 */
internal class PathNormalizer(private val dx: Float, private val dy: Float) {
    fun normalize(data: String): String {
        val tokens = PathTokens(data)
        val out = StringBuilder()
        var x = 0.0
        var y = 0.0
        var startX = 0.0
        var startY = 0.0
        // The previous segment's last control point, for the smooth S and T commands.
        var controlX = 0.0
        var controlY = 0.0
        var previous = ' '
        var command = ' '

        fun emit(letter: Char, vararg values: Double) {
            if (out.isNotEmpty()) out.append(' ')
            out.append(letter)
            values.forEachIndexed { index, value ->
                out.append(' ')
                out.append(format(value + if (index % 2 == 0) dx else dy))
            }
        }

        fun cubic(x1: Double, y1: Double, x2: Double, y2: Double, endX: Double, endY: Double) {
            emit('C', x1, y1, x2, y2, endX, endY)
            controlX = x2
            controlY = y2
            x = endX
            y = endY
        }

        while (tokens.hasMore()) {
            command = tokens.command() ?: command
            require(command != ' ') { "path data must start with a command" }
            val relative = command.isLowerCase()
            val baseX = if (relative) x else 0.0
            val baseY = if (relative) y else 0.0
            when (command.uppercaseChar()) {
                'M' -> {
                    x = baseX + tokens.number()
                    y = baseY + tokens.number()
                    startX = x
                    startY = y
                    emit('M', x, y)
                    // Further coordinate pairs after a move are lines.
                    command = if (relative) 'l' else 'L'
                }
                'L' -> {
                    x = baseX + tokens.number()
                    y = baseY + tokens.number()
                    emit('L', x, y)
                }
                'H' -> {
                    x = baseX + tokens.number()
                    emit('L', x, y)
                }
                'V' -> {
                    y = baseY + tokens.number()
                    emit('L', x, y)
                }
                'C' ->
                    cubic(
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                    )
                'S' -> {
                    val smooth = previous.uppercaseChar() in "CS"
                    cubic(
                        if (smooth) 2 * x - controlX else x,
                        if (smooth) 2 * y - controlY else y,
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                    )
                }
                'Q' -> {
                    val qx = baseX + tokens.number()
                    val qy = baseY + tokens.number()
                    quadratic(
                        qx,
                        qy,
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        x,
                        y,
                        ::cubic,
                    )
                    controlX = qx
                    controlY = qy
                }
                'T' -> {
                    val smooth = previous.uppercaseChar() in "QT"
                    val qx = if (smooth) 2 * x - controlX else x
                    val qy = if (smooth) 2 * y - controlY else y
                    quadratic(
                        qx,
                        qy,
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        x,
                        y,
                        ::cubic,
                    )
                    controlX = qx
                    controlY = qy
                }
                'A' -> {
                    val rx = tokens.number()
                    val ry = tokens.number()
                    val rotation = tokens.number()
                    val large = tokens.flag()
                    val sweep = tokens.flag()
                    arc(
                        x,
                        y,
                        rx,
                        ry,
                        rotation,
                        large,
                        sweep,
                        baseX + tokens.number(),
                        baseY + tokens.number(),
                        ::cubic,
                    )
                }
                'Z' -> {
                    emit('Z')
                    x = startX
                    y = startY
                }
                else -> throw IllegalArgumentException("unknown path command $command")
            }
            previous = command
        }
        return out.toString()
    }

    private fun format(value: Double): String {
        val rounded = Math.round(value * 1000) / 1000.0
        return if (rounded == floor(rounded)) rounded.toLong().toString() else rounded.toString()
    }
}
