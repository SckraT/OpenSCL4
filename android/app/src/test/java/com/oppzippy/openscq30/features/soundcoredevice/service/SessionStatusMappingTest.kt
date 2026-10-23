package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import com.oppzippy.openscq30.lib.bindings.SessionStatus
import org.junit.Assert.assertEquals
import org.junit.Test

class SessionStatusMappingTest {
    @Test
    fun nativeSessionStatesMapToTheControllerStates() {
        assertEquals(SessionConnectionState.Connected, SessionStatus.Connected.toSessionConnectionState())
        assertEquals(
            SessionConnectionState.Recovering(attempt = 2, reason = SessionFailureReason.TRANSPORT_LOST),
            SessionStatus.Recovering(
                attempt = 2u,
                reason = SessionFailureReason.TRANSPORT_LOST,
            ).toSessionConnectionState(),
        )
        assertEquals(
            SessionConnectionState.Lost(SessionFailureReason.CONNECT_TIMED_OUT),
            SessionStatus.Lost(SessionFailureReason.CONNECT_TIMED_OUT).toSessionConnectionState(),
        )
        assertEquals(SessionConnectionState.Closed, SessionStatus.Closed.toSessionConnectionState())
    }
}
