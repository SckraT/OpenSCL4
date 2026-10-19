package com.oppzippy.openscq30.gradle

import java.io.File
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

abstract class GenerateUniffiBindingsTask @Inject constructor(
    private val execOps: ExecOperations,
) : DefaultTask() {
    @get:Input
    abstract val rustWorkspaceDirectory: Property<File>

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @get:InputFile
    abstract val nativeLibraryFile: RegularFileProperty

    @get:InputFile
    abstract val uniffiConfigFile: RegularFileProperty

    @TaskAction
    fun generate() {
        execOps.exec {
            workingDir = rustWorkspaceDirectory.get()
            commandLine(
                "cargo",
                "run",
                "--locked",
                "--bin",
                "uniffi-bindgen",
                "--",
                "generate",
                "--library",
                nativeLibraryFile.get().asFile.absolutePath,
                "--language",
                "kotlin",
                "--out-dir",
                outputDirectory.get().asFile.absolutePath,
                "--no-format",
            )
        }
    }
}
