package com.oppzippy.openscq30.gradle

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.gradle.api.GradleException

/** AGP writes one result file per device, named after the device. */
private const val RESULT_FILE_PREFIX = "TEST-"

/**
 * Fails when a connected test task's results show a device that ran no tests. AGP 9.4 reports an instrumentation
 * that dies before its first test as a passing run with zero tests, so the task alone can succeed untested.
 */
fun requireConnectedTestsRan(resultsDir: File) {
    val results = resultsDir.walk()
        .filter { it.isFile && it.name.startsWith(RESULT_FILE_PREFIX) && it.extension == "xml" }
        .toList()
    if (results.isEmpty()) {
        throw GradleException("No instrumented test results were written to $resultsDir.")
    }
    val devicesWithoutTests = results.filter { testCount(it) == 0 }
        .map { it.nameWithoutExtension.removePrefix(RESULT_FILE_PREFIX) }
    if (devicesWithoutTests.isNotEmpty()) {
        throw GradleException(
            "No instrumented tests ran on ${devicesWithoutTests.joinToString()}. Either the test filter matched " +
                "nothing or the instrumentation crashed before its first test, which AGP reports as a passing " +
                "run; check `adb logcat -b crash -d` on that device.",
        )
    }
}

private fun testCount(result: File): Int =
    DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(result)
        .documentElement.getAttribute("tests").toInt()
