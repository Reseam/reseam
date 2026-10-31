// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.settings

/** How a brand and its vectors are written into the settings schema. */
internal fun Json.brand(brand: Brand, font: String?) = obj {
    field("name", brand.name)
    field("light") { colors(brand.light) }
    field("dark") { colors(brand.dark) }
    field("logo") {
        val logo = brand.logo
        if (logo == null) nothing()
        else
            obj {
                field("light") { vector(logo.light) }
                field("dark") { vector(logo.dark) }
            }
    }
    field("font", font)
    field("typography") {
        val type = brand.typography
        obj {
            field("headlineSmall") { style(type.headlineSmall) }
            field("titleLarge") { style(type.titleLarge) }
            field("titleMedium") { style(type.titleMedium) }
            field("bodyLarge") { style(type.bodyLarge) }
            field("bodyMedium") { style(type.bodyMedium) }
            field("labelLarge") { style(type.labelLarge) }
            field("labelMedium") { style(type.labelMedium) }
        }
    }
}

private fun Json.colors(scheme: ColorScheme) = obj {
    field("primary", scheme.primary)
    field("onPrimary", scheme.onPrimary)
    field("primaryContainer", scheme.primaryContainer)
    field("onPrimaryContainer", scheme.onPrimaryContainer)
    field("secondaryContainer", scheme.secondaryContainer)
    field("onSecondaryContainer", scheme.onSecondaryContainer)
    field("surface", scheme.surface)
    field("surfaceContainerLow", scheme.surfaceContainerLow)
    field("surfaceContainer", scheme.surfaceContainer)
    field("surfaceContainerHigh", scheme.surfaceContainerHigh)
    field("surfaceContainerHighest", scheme.surfaceContainerHighest)
    field("onSurface", scheme.onSurface)
    field("onSurfaceVariant", scheme.onSurfaceVariant)
    field("outline", scheme.outline)
    field("outlineVariant", scheme.outlineVariant)
    field("error", scheme.error)
    field("onError", scheme.onError)
    field("errorContainer", scheme.errorContainer)
    field("onErrorContainer", scheme.onErrorContainer)
    field("scrim", scheme.scrim)
}

private fun Json.style(style: TextStyle) = obj {
    field("size", style.size)
    field("lineHeight", style.lineHeight)
    field("weight", style.weight)
    field("tracking", style.tracking)
}

internal fun Json.vector(vector: Vector?) {
    if (vector == null) return nothing()
    obj {
        field("width", vector.width)
        field("height", vector.height)
        field("paths") {
            array(vector.paths) { path ->
                obj {
                    field("data", path.data)
                    field("fill") {
                        val fill = path.fill
                        if (fill == null) nothing()
                        else
                            obj {
                                field("paint") { paint(fill.paint) }
                                field("evenOdd", fill.evenOdd)
                            }
                    }
                    field("stroke") {
                        val stroke = path.stroke
                        if (stroke == null) nothing()
                        else
                            obj {
                                field("paint") { paint(stroke.paint) }
                                field("width", stroke.width)
                                field("cap", stroke.cap.name.lowercase())
                                field("join", stroke.join.name.lowercase())
                            }
                    }
                }
            }
        }
    }
}

/** `null` is the color the screen draws the vector in. */
private fun Json.paint(paint: Paint) =
    when (paint) {
        Paint.Current -> nothing()
        is Paint.Solid -> value(paint.color.toString())
    }

private fun Json.field(name: String, color: Color) = field(name, color.toString())
