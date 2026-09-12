// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.GradleException
import org.gradle.api.artifacts.transform.InputArtifact
import org.gradle.api.artifacts.transform.TransformAction
import org.gradle.api.artifacts.transform.TransformOutputs
import org.gradle.api.artifacts.transform.TransformParameters
import org.gradle.api.attributes.Attribute
import org.gradle.api.attributes.AttributeCompatibilityRule
import org.gradle.api.attributes.CompatibilityCheckDetails
import org.gradle.api.attributes.LibraryElements
import org.gradle.api.file.FileSystemLocation
import org.gradle.api.provider.Provider
import java.util.zip.ZipFile

val ARTIFACT_TYPE: Attribute<String> = Attribute.of("artifactType", String::class.java)

/** An Android library's `aar` elements satisfy a consumer asking for a jar; the transform below delivers one. */
class AarElementsCompatibility : AttributeCompatibilityRule<LibraryElements> {
    override fun execute(details: CompatibilityCheckDetails<LibraryElements>) {
        if (details.producerValue?.name == "aar" && details.consumerValue?.name in setOf(LibraryElements.JAR, LibraryElements.CLASSES)) details.compatible()
    }
}

/**
 * The jars inside an AAR, so an extension can depend on Android libraries
 * without the Android Gradle Plugin. Resources and the manifest have no place
 * in a DEX merged into another app and are left behind; native libraries
 * would fail at runtime, so an AAR carrying them is refused.
 */
abstract class AarClassesTransform : TransformAction<TransformParameters.None> {
    @get:InputArtifact
    abstract val input: Provider<FileSystemLocation>

    override fun transform(outputs: TransformOutputs) {
        val aar = input.get().asFile
        ZipFile(aar).use { zip ->
            val entries = zip.entries().asSequence().filter { !it.isDirectory }.toList()
            if (entries.any { it.name.startsWith("jni/") }) {
                throw GradleException("${aar.name} ships native libraries, which an extension cannot carry")
            }
            for (entry in entries.filter { it.name == "classes.jar" || (it.name.startsWith("libs/") && it.name.endsWith(".jar")) }) {
                val name = aar.nameWithoutExtension + "-" + entry.name.substringAfterLast('/')
                zip.getInputStream(entry).use { stream -> outputs.file(name).outputStream().use(stream::copyTo) }
            }
        }
    }
}
