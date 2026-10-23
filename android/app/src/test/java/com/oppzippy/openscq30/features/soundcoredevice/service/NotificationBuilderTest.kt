package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import io.mockk.mockk
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NotificationBuilderTest {
    private val manager = mockk<DeviceConnectionManager>(relaxed = true)

    @Test
    fun retryActionIsNotOfferedWhileRecovering() {
        assertFalse(
            ConnectionStatus.Recovering(
                manager,
                "AA:BB",
                attempt = 1,
                reason = SessionFailureReason.TRANSPORT_LOST,
            ).offersRetryAction(),
        )
    }

    @Test
    fun retryActionIsOfferedWhenLost() {
        assertTrue(
            ConnectionStatus.Lost(
                manager,
                "AA:BB",
                reason = SessionFailureReason.CONNECT_TIMED_OUT,
            ).offersRetryAction(),
        )
    }

    @Test
    fun retryActionIsNotOfferedWhenConnectedOrDisconnected() {
        assertFalse(ConnectionStatus.Connected(manager, "AA:BB").offersRetryAction())
        assertFalse(ConnectionStatus.Disconnected.offersRetryAction())
    }
}
