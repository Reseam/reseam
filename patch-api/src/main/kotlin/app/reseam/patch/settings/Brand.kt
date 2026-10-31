// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/**
 * How a bundle looks in the app: its settings screen and the components its extensions build with
 * the app runtime. The screen follows the system's light or dark theme. A host without a brand uses
 * [Brand.DEFAULT].
 *
 * ```kotlin
 * private object Files
 *
 * val brand = Brand(
 *     name = "Example",
 *     light = ColorScheme(primary = Color.hex("#296A48"), ...),
 *     dark = ColorScheme(primary = Color.hex("#A2EBBD"), ...),
 *     logo = Logo(Vector.svg(Files::class.java.getResource("/brand/logo.svg")!!.readText())),
 *     font = Font(Files::class.java.getResource("/brand/Inter.ttf")!!.readBytes()),
 * )
 * ```
 */
class Brand(
    val name: String,
    val light: ColorScheme,
    val dark: ColorScheme,
    val logo: Logo? = null,
    val font: Font? = null,
    val typography: Typography = Typography(),
) {
    init {
        require(name.isNotBlank()) { "a brand needs a name" }
    }

    companion object {
        /** Material 3's baseline colors and type in the system font, without a logo. */
        val DEFAULT =
            Brand(
                name = "Settings",
                light =
                    ColorScheme(
                        primary = Color.hex("#6750A4"),
                        onPrimary = Color.hex("#FFFFFF"),
                        primaryContainer = Color.hex("#EADDFF"),
                        onPrimaryContainer = Color.hex("#4F378B"),
                        secondaryContainer = Color.hex("#E8DEF8"),
                        onSecondaryContainer = Color.hex("#4A4458"),
                        surface = Color.hex("#FEF7FF"),
                        surfaceContainerLow = Color.hex("#F7F2FA"),
                        surfaceContainer = Color.hex("#F3EDF7"),
                        surfaceContainerHigh = Color.hex("#ECE6F0"),
                        surfaceContainerHighest = Color.hex("#E6E0E9"),
                        onSurface = Color.hex("#1D1B20"),
                        onSurfaceVariant = Color.hex("#49454F"),
                        outline = Color.hex("#79747E"),
                        outlineVariant = Color.hex("#CAC4D0"),
                        error = Color.hex("#B3261E"),
                        onError = Color.hex("#FFFFFF"),
                        errorContainer = Color.hex("#F9DEDC"),
                        onErrorContainer = Color.hex("#8C1D18"),
                        scrim = Color.hex("#000000"),
                    ),
                dark =
                    ColorScheme(
                        primary = Color.hex("#D0BCFF"),
                        onPrimary = Color.hex("#381E72"),
                        primaryContainer = Color.hex("#4F378B"),
                        onPrimaryContainer = Color.hex("#EADDFF"),
                        secondaryContainer = Color.hex("#4A4458"),
                        onSecondaryContainer = Color.hex("#E8DEF8"),
                        surface = Color.hex("#141218"),
                        surfaceContainerLow = Color.hex("#1D1B20"),
                        surfaceContainer = Color.hex("#211F26"),
                        surfaceContainerHigh = Color.hex("#2B2930"),
                        surfaceContainerHighest = Color.hex("#36343B"),
                        onSurface = Color.hex("#E6E0E9"),
                        onSurfaceVariant = Color.hex("#CAC4D0"),
                        outline = Color.hex("#938F99"),
                        outlineVariant = Color.hex("#49454F"),
                        error = Color.hex("#F2B8B5"),
                        onError = Color.hex("#601410"),
                        errorContainer = Color.hex("#8C1D18"),
                        onErrorContainer = Color.hex("#F9DEDC"),
                        scrim = Color.hex("#000000"),
                    ),
            )
    }
}

/** An sRGB color. */
class Color private constructor(internal val argb: Int) {
    override fun equals(other: Any?) = other is Color && other.argb == argb

    override fun hashCode() = argb

    override fun toString() = "#%08X".format(argb)

    companion object {
        /** Parses `#rgb`, `#rrggbb`, or `#aarrggbb` with alpha first. */
        fun hex(value: String): Color {
            val digits = value.removePrefix("#")
            require(
                value.startsWith("#") &&
                    digits.all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }
            ) {
                "\"$value\" is not a #rgb, #rrggbb or #aarrggbb color"
            }
            val argb =
                when (digits.length) {
                    3 -> "FF" + digits.map { "$it$it" }.joinToString("")
                    6 -> "FF$digits"
                    8 -> digits
                    else ->
                        throw IllegalArgumentException(
                            "\"$value\" is not a #rgb, #rrggbb or #aarrggbb color"
                        )
                }
            return Color(argb.toLong(16).toInt())
        }
    }
}

/** The Material 3 color roles the app runtime paints with. */
class ColorScheme(
    val primary: Color,
    val onPrimary: Color,
    val primaryContainer: Color,
    val onPrimaryContainer: Color,
    val secondaryContainer: Color,
    val onSecondaryContainer: Color,
    val surface: Color,
    val surfaceContainerLow: Color,
    val surfaceContainer: Color,
    val surfaceContainerHigh: Color,
    val surfaceContainerHighest: Color,
    val onSurface: Color,
    val onSurfaceVariant: Color,
    val outline: Color,
    val outlineVariant: Color,
    val error: Color,
    val onError: Color,
    val errorContainer: Color,
    val onErrorContainer: Color,
    val scrim: Color,
)

/**
 * The Material 3 type roles the app runtime sets text in. The defaults are Material 3's baseline
 * sizes and weights.
 */
class Typography(
    val headlineSmall: TextStyle = TextStyle(size = 24f, lineHeight = 32f, weight = 400),
    val titleLarge: TextStyle = TextStyle(size = 22f, lineHeight = 28f, weight = 400),
    val titleMedium: TextStyle =
        TextStyle(size = 16f, lineHeight = 24f, weight = 500, tracking = 0.15f),
    val bodyLarge: TextStyle =
        TextStyle(size = 16f, lineHeight = 24f, weight = 400, tracking = 0.5f),
    val bodyMedium: TextStyle =
        TextStyle(size = 14f, lineHeight = 20f, weight = 400, tracking = 0.25f),
    val labelLarge: TextStyle =
        TextStyle(size = 14f, lineHeight = 20f, weight = 500, tracking = 0.1f),
    val labelMedium: TextStyle =
        TextStyle(size = 12f, lineHeight = 16f, weight = 500, tracking = 0.5f),
)

/** Text in [size] and [lineHeight] sp, a [weight] from 1 to 1000, and [tracking] in sp. */
class TextStyle(val size: Float, val lineHeight: Float, val weight: Int, val tracking: Float = 0f) {
    init {
        require(size > 0 && lineHeight >= size) {
            "a text style needs a line height of at least its size"
        }
        require(weight in 1..1000) { "font weights run from 1 to 1000, not $weight" }
    }
}

/**
 * A TrueType or OpenType font file, written into the app. A variable font with a `wght` axis sets
 * every [TextStyle.weight]; a static font renders its one weight.
 */
class Font(internal val file: ByteArray) {
    init {
        require(file.size >= 4) { "a font file is at least a header" }
    }
}

/** A brand's mark, for the light and the dark theme. */
class Logo(val light: Vector, val dark: Vector = light)
