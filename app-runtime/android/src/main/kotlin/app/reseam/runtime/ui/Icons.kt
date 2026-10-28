// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-FileCopyrightText: 2026 Lucide Icons and Contributors
// SPDX-License-Identifier: AGPL-3.0-or-later AND ISC

package app.reseam.runtime.ui

object Icons {
    @JvmField val ARROW_LEFT = VectorImage.stroked("M 12 19 L 5 12 L 12 5", "M 19 12 L 5 12")
    @JvmField val CHEVRON_RIGHT = VectorImage.stroked("M 9 18 L 15 12 L 9 6")
    @JvmField
    val SEARCH =
        VectorImage.stroked(
            "M 21 21 L 16.66 16.66",
            "M 3 11 C 3 15.418 6.582 19 11 19 C 15.418 19 19 15.418 19 11 C 19 6.582 15.418 3 11 3 " +
                "C 6.582 3 3 6.582 3 11 Z",
        )
    @JvmField val CLOSE = VectorImage.stroked("M 18 6 L 6 18", "M 6 6 L 18 18")
    @JvmField val CHECK = VectorImage.stroked("M 20 6 L 9 17 L 4 12")
    @JvmField
    val RESET =
        VectorImage.stroked(
            "M 3 12 C 3 16.971 7.029 21 12 21 C 16.971 21 21 16.971 21 12 C 21 7.029 16.971 3 12 3 " +
                "C 9.484 3.009 7.069 3.991 5.26 5.74 L 3 8",
            "M 3 3 L 3 8 L 8 8",
        )
    @JvmField
    val SUN =
        VectorImage.stroked(
            "M 16 12 C 16 14.209 14.209 16 12 16 C 9.791 16 8 14.209 8 12 C 8 9.791 9.791 8 12 8 " +
                "C 14.209 8 16 9.791 16 12 Z",
            "M 12 2 L 12 4",
            "M 12 20 L 12 22",
            "M 4.93 4.93 L 6.34 6.34",
            "M 17.66 17.66 L 19.07 19.07",
            "M 2 12 L 4 12",
            "M 20 12 L 22 12",
            "M 6.34 17.66 L 4.93 19.07",
            "M 19.07 4.93 L 17.66 6.34",
        )
    @JvmField
    val MOON =
        VectorImage.stroked(
            "M 20.985 12.486 C 20.724 17.323 16.68 21.086 11.837 20.999 C 6.993 20.913 3.087 17.007 " +
                "3 12.163 C 2.912 7.32 6.675 3.276 11.512 3.014 C 11.917 2.992 12.129 3.474 11.914 " +
                "3.817 C 10.433 6.186 10.784 9.264 12.759 11.24 C 14.735 13.215 17.813 13.566 20.182 " +
                "12.085 C 20.526 11.87 21.007 12.081 20.985 12.486"
        )
    @JvmField
    val UPLOAD =
        VectorImage.stroked(
            TRAY,
            "M 17 8 L 12 3 L 7 8",
            "M 12 3 L 12 15",
        )
    @JvmField
    val DOWNLOAD =
        VectorImage.stroked(
            TRAY,
            "M 7 10 L 12 15 L 17 10",
            "M 12 15 L 12 3",
        )
}

private const val TRAY =
    "M 21 15 L 21 19 C 21 20.105 20.105 21 19 21 L 5 21 C 3.895 21 3 20.105 3 19 L 3 15"
