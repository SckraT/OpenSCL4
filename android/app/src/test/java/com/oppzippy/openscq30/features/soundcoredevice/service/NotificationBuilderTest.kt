package com.oppzippy.openscq30.features.soundcoredevice.service

import io.mockk.mockk
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NotificationBuilderTest {
    private val manager = mockk<DeviceConnectionManager>(relaxed = true)

    @Test
    fun retryActionIsOfferedWhileRecovering() {
        assertTrue(
            ConnectionStatus.Recovering(manager, "AA:BB", attempt = 1, reason = "transport lost").offersRetryAction(),
        )
    }

    @Test
    fun retryActionIsOfferedWhenLost() {
        assertTrue(ConnectionStatus.Lost(manager, "AA:BB", reason = "recovery budget exhausted").offersRetryAction())
    }

    @Test
    fun retryActionIsNotOfferedWhenConnectedOrDisconnected() {
        assertFalse(ConnectionStatus.Connected(manager, "AA:BB").offersRetryAction())
        assertFalse(ConnectionStatus.Disconnected.offersRetryAction())
    }
}
