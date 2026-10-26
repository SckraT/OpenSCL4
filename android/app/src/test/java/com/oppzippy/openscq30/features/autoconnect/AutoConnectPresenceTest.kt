package com.oppzippy.openscq30.features.autoconnect

import android.companion.DeviceNotAssociatedException
import android.util.Log
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

class AutoConnectPresenceTest {
    @Before
    fun setUp() {
        mockkStatic(Log::class)
        every { Log.w(any<String>(), any<String>(), any<Throwable>()) } returns 0
    }

    @After
    fun tearDown() {
        unmockkStatic(Log::class)
    }

    @Test
    fun observesEveryAssociatedDevice() {
        val observed = mutableListOf<String>()

        val count = observeCompanionPresence(listOf("aa:bb", "cc:dd")) { observed += it }

        assertEquals(listOf("aa:bb", "cc:dd"), observed)
        assertEquals(2, count)
    }

    @Test
    fun keepsObservingWhenOneAssociationWasRemovedMeanwhile() {
        val observed = mutableListOf<String>()

        val count = observeCompanionPresence(listOf("aa:bb", "cc:dd", "ee:ff")) { address ->
            if (address == "cc:dd") throw mockk<DeviceNotAssociatedException>()
            observed += address
        }

        assertEquals(listOf("aa:bb", "ee:ff"), observed)
        assertEquals(2, count)
    }

    @Test
    fun keepsObservingWithoutThePresencePermission() {
        val observed = mutableListOf<String>()

        val count = observeCompanionPresence(listOf("aa:bb", "cc:dd")) { address ->
            if (address == "aa:bb") throw SecurityException("missing REQUEST_OBSERVE_COMPANION_DEVICE_PRESENCE")
            observed += address
        }

        assertEquals(listOf("cc:dd"), observed)
        assertEquals(1, count)
    }

    @Test
    fun noAssociationsRegistersNothing() {
        val count = observeCompanionPresence(emptyList()) { fail("nothing to observe") }

        assertEquals(0, count)
    }
}
