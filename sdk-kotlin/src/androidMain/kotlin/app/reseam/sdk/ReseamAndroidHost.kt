// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.sdk

object ReseamAndroidHost {
    init {
        System.loadLibrary("reseam_sdk")
    }

    @JvmStatic
    external fun setClassLoader(classLoader: ClassLoader)
}
