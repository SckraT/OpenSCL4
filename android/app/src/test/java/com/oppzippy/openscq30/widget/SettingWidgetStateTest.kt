package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import io.mockk.mockk
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SettingWidgetStateTest {
    private val manager = mockk<DeviceConnectionManager>(relaxed = true)

    @Test
    fun recoveringMapsToTheRecoveringWidgetState() {
        assertEquals(
            SettingWidgetState.Recovering("Liberty 4 Pro"),
            recoveryWidgetState(
                ConnectionStatus.Recovering(
                    manager,
                    "AA:BB",
                    attempt = 1,
                    reason = SessionFailureReason.TRANSPORT_LOST,
                ),
                "Liberty 4 Pro",
            ),
        )
    }

    @Test
    fun lostMapsToTheLostWidgetStateWithItsMacAddress() {
        assertEquals(
            SettingWidgetState.Lost("Liberty 4 Pro", "AA:BB"),
            recoveryWidgetState(
                ConnectionStatus.Lost(manager, "AA:BB", reason = SessionFailureReason.CONNECT_TIMED_OUT),
                "Liberty 4 Pro",
            ),
        )
    }

    @Test
    fun connectedIsNotAWidgetRecoveryState() {
        assertNull(recoveryWidgetState(ConnectionStatus.Connected(manager, "AA:BB"), "Liberty 4 Pro"))
    }

    @Test
    fun recoveryStatesSurviveWidgetStateSerialization() {
        listOf(
            SettingWidgetState.Recovering("Liberty 4 Pro"),
            SettingWidgetState.Lost("Liberty 4 Pro", "AA:BB"),
        ).forEach { state ->
            assertEquals(state, Json.decodeFromString<SettingWidgetState>(Json.encodeToString(state)))
        }
    }
}
