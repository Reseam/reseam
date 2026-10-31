// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.runtime.ui

import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import java.util.Locale
import org.json.JSONObject

/** A settings vector using the absolute move, line, cubic and close commands written by the SDK. */
class VectorImage
private constructor(
    internal val width: Float,
    internal val height: Float,
    internal val shapes: List<Shape>,
) {
    companion object {
        internal fun stroked(vararg data: String): VectorImage =
            VectorImage(
                24f,
                24f,
                data.map {
                    Shape(
                        path(it),
                        null,
                        Stroke(Ink(null), 2f, Paint.Cap.ROUND, Paint.Join.ROUND),
                        false,
                    )
                },
            )

        /** Reads the SDK form; null means no image. */
        @JvmStatic
        fun read(json: JSONObject?): VectorImage? {
            if (json == null) return null
            val paths = json.getJSONArray("paths")
            val shapes = List(paths.length()) { paths.getJSONObject(it).shape() }
            return VectorImage(
                json.getDouble("width").toFloat(),
                json.getDouble("height").toFloat(),
                shapes,
            )
        }
    }

    internal data class Ink(private val color: Int?) {
        fun resolve(tint: Int): Int = color ?: tint
    }

    internal class Shape(val path: Path, val fill: Ink?, val stroke: Stroke?, evenOdd: Boolean) {
        init {
            path.fillType = if (evenOdd) Path.FillType.EVEN_ODD else Path.FillType.WINDING
        }
    }

    internal data class Stroke(
        val ink: Ink,
        val width: Float,
        val cap: Paint.Cap,
        val join: Paint.Join,
    )
}

private fun JSONObject.ink(): VectorImage.Ink =
    VectorImage.Ink(if (isNull("paint")) null else Color.parseColor(getString("paint")))

private fun JSONObject.shape(): VectorImage.Shape {
    val fill = optJSONObject("fill")
    val stroke = optJSONObject("stroke")
    return VectorImage.Shape(
        path(getString("data")),
        fill?.ink(),
        stroke?.let {
            VectorImage.Stroke(
                it.ink(),
                it.getDouble("width").toFloat(),
                Paint.Cap.valueOf(it.getString("cap").uppercase(Locale.ROOT)),
                Paint.Join.valueOf(it.getString("join").uppercase(Locale.ROOT)),
            )
        },
        fill?.getBoolean("evenOdd") ?: false,
    )
}

private fun path(data: String): Path {
    val path = Path()
    val tokens = data.trim().split(" ")
    var index = 0
    fun coordinate(): Float = tokens[index++].toFloat()
    while (index < tokens.size) {
        when (val command = tokens[index++]) {
            "M" -> path.moveTo(coordinate(), coordinate())
            "L" -> path.lineTo(coordinate(), coordinate())
            "C" ->
                path.cubicTo(
                    coordinate(),
                    coordinate(),
                    coordinate(),
                    coordinate(),
                    coordinate(),
                    coordinate(),
                )
            "Z" -> path.close()
            else -> throw IllegalArgumentException("Unexpected path command $command")
        }
    }
    return path
}
