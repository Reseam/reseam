// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.sdk

/** Supplies the Android loader that owns this engine version's shared patch runtime. */
object ReseamAndroidHost {
    init {
        System.loadLibrary("reseam-sdk-native")
    }

    /**
     * Installs the parent for bundle class loaders before inspecting or applying bundles. The
     * loader must be able to load `reseam-patch-sdk` from the same engine version. Native
     * installation failures throw [IllegalStateException].
     */
    @JvmStatic external fun setClassLoader(classLoader: ClassLoader)
}
