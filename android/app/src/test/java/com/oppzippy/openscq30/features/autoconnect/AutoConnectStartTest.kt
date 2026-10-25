package com.oppzippy.openscq30.features.autoconnect

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Test

class AutoConnectStartTest {
    @Test
    fun refusedBackgroundStartIsReportedInsteadOfThrown() {
        // BackgroundServiceStartNotAllowedException is an IllegalStateException; Android 8-11 throw the base class.
        val refusal = IllegalStateException("Not allowed to start service")
        val refused = mutableListOf<IllegalStateException>()

        val started = startUnlessRefused(start = { throw refusal }, onRefused = { refused += it })

        assertFalse(started)
        assertEquals(listOf(refusal), refused)
    }

    @Test
    fun otherStartFailuresStillThrow() {
        assertThrows(SecurityException::class.java) {
            startUnlessRefused(start = { throw SecurityException("missing permission") }, onRefused = {})
        }
    }
}
