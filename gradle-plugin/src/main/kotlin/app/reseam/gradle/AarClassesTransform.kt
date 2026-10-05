// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import java.util.zip.ZipFile
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

internal val ARTIFACT_TYPE: Attribute<String> = Attribute.of("artifactType", String::class.java)

/** Android libraries publish one variant per build type; extensions build against release. */
internal val BUILD_TYPE: Attribute<String> =
    Attribute.of("com.android.build.api.attributes.BuildTypeAttr", String::class.java)

internal class AarElementsCompatibility : AttributeCompatibilityRule<LibraryElements> {
    override fun execute(details: CompatibilityCheckDetails<LibraryElements>) {
        if (
            details.producerValue?.name == "aar" &&
                details.consumerValue?.name in setOf(LibraryElements.JAR, LibraryElements.CLASSES)
        )
            details.compatible()
    }
}

internal abstract class AarClassesTransform : TransformAction<TransformParameters.None> {
    @get:InputArtifact abstract val input: Provider<FileSystemLocation>

    override fun transform(outputs: TransformOutputs) {
        val aar = input.get().asFile
        ZipFile(aar).use { zip ->
            for (entry in
                zip.entries().asSequence().filter {
                    it.name == "classes.jar" ||
                        (it.name.startsWith("libs/") && it.name.endsWith(".jar"))
                }) {
                val name = aar.nameWithoutExtension + "-" + entry.name.substringAfterLast('/')
                zip.getInputStream(entry).use { stream ->
                    outputs.file(name).outputStream().use(stream::copyTo)
                }
            }
        }
    }
}
