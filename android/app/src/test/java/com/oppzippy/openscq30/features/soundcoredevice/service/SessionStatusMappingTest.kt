package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.SessionStatus
import org.junit.Assert.assertEquals
import org.junit.Test

class SessionStatusMappingTest {
    @Test
    fun nativeSessionStatesMapToTheControllerStates() {
        assertEquals(SessionConnectionState.Connected, SessionStatus.Connected.toSessionConnectionState())
        assertEquals(
            SessionConnectionState.Recovering(attempt = 2, reason = "transport lost"),
            SessionStatus.Recovering(attempt = 2u, reason = "transport lost").toSessionConnectionState(),
        )
        assertEquals(
            SessionConnectionState.Lost("recovery budget exhausted"),
            SessionStatus.Lost("recovery budget exhausted").toSessionConnectionState(),
        )
        assertEquals(SessionConnectionState.Closed, SessionStatus.Closed.toSessionConnectionState())
    }
}
