package com.oppzippy.openscq30.gradle

import java.io.File
import org.gradle.api.GradleException
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ConnectedTestResultsTest {
    @get:Rule
    val resultsDir = TemporaryFolder()

    @Test
    fun aDeviceThatRanNoTestsFailsTheRun() {
        writeResult("Pixel 8 Pro - 16", tests = 0)

        val error = assertThrows(GradleException::class.java) { requireConnectedTestsRan(resultsDir.root) }

        assertTrue(error.message!!, error.message!!.contains("Pixel 8 Pro - 16"))
    }

    @Test
    fun onlyTheDeviceThatRanNoTestsIsNamed() {
        writeResult("Pixel 8 Pro - 16", tests = 0)
        writeResult("Pixel_8_Pro(AVD) - 17", tests = 30)

        val error = assertThrows(GradleException::class.java) { requireConnectedTestsRan(resultsDir.root) }

        assertFalse(error.message!!, error.message!!.contains("Pixel_8_Pro(AVD) - 17"))
    }

    @Test
    fun aRunWithTestsOnEveryDevicePasses() {
        writeResult("Pixel 8 Pro - 16", tests = 30)
        writeResult("Pixel_8_Pro(AVD) - 17", tests = 30)

        requireConnectedTestsRan(resultsDir.root)
    }

    @Test
    fun aRunWithoutResultsFails() {
        assertThrows(GradleException::class.java) { requireConnectedTestsRan(resultsDir.root) }
    }

    private fun writeResult(device: String, tests: Int) {
        File(resultsDir.root, "TEST-$device.xml").writeText(
            """
            <?xml version='1.0' encoding='UTF-8' ?>
            <testsuites tests="$tests" failures="0" errors="0" skipped="0" time="0.000" hostname="localhost" />
            """.trimIndent(),
        )
    }
}
