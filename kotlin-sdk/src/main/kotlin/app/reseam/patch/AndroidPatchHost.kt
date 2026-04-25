// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.patch

/**
 * Android-only host controls for Kotlin patch loading.
 *
 * Manager applications should install the ClassLoader that can resolve the
 * Reseam SDK and patch classes before invoking bundle inspection or patching.
 * Desktop hosts continue to use the JVM URLClassLoader path.
 */
object AndroidPatchHost {
    init {
        val vmName = System.getProperty("java.vm.name").orEmpty()
        val isAndroidRuntime =
            vmName.contains("dalvik", ignoreCase = true) ||
                vmName.contains("art", ignoreCase = true)
        if (isAndroidRuntime) {
            System.loadLibrary("reseam_patcher")
        }
    }

    @JvmStatic
    external fun setClassLoader(classLoader: ClassLoader)

    @JvmStatic
    external fun clearClassLoader()
}
