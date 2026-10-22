package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import io.mockk.mockk
import org.junit.Assert.assertEquals
import org.junit.Test

class AppControlSnapshotTest {
    private val mac = "AA:BB:CC:DD:EE:FF"

    @Test
    fun aConnectedActiveDeviceMapsToControlling() {
        assertEquals(
            AppControlSnapshot(mac, AppControl.Controlling),
            ActiveDevice(mac, ActiveDeviceState.Connected).toAppControlSnapshot(),
        )
    }

    @Test
    fun aConnectingActiveDeviceMapsToConnecting() {
        assertEquals(
            AppControlSnapshot(mac, AppControl.Connecting),
            ActiveDevice(mac, ActiveDeviceState.Connecting).toAppControlSnapshot(),
        )
    }

    @Test
    fun noActiveDeviceMeansNoControl() {
        assertEquals(AppControlSnapshot(), null.toAppControlSnapshot())
    }

    @Test
    fun theServiceConnectionStateMapsToTheAppControlAxis() {
        val manager = mockk<DeviceConnectionManager>(relaxed = true)

        assertEquals(
            AppControlSnapshot(mac, AppControl.Controlling),
            ConnectionStatus.Connected(manager, mac).toAppControlSnapshot(),
        )
        assertEquals(
            AppControlSnapshot(mac, AppControl.Connecting),
            ConnectionStatus.Connecting(mac).toAppControlSnapshot(),
        )
        assertEquals(
            AppControlSnapshot(mac, AppControl.Connecting),
            ConnectionStatus.Recovering(manager, mac, attempt = 2, reason = "transport lost").toAppControlSnapshot(),
        )
        assertEquals(
            AppControlSnapshot(mac, AppControl.Controlling),
            ConnectionStatus.Lost(manager, mac, "recovery budget exhausted").toAppControlSnapshot(),
        )
        assertEquals(AppControlSnapshot(), ConnectionStatus.Disconnected.toAppControlSnapshot())
    }

    @Test
    fun onlyTheControlledDeviceKeepsItsPhase() {
        val snapshot = AppControlSnapshot("AA:BB:CC:DD:EE:FF", AppControl.Controlling)

        assertEquals(AppControl.Controlling, snapshot.phaseOf("aa:bb:cc:dd:ee:ff"))
        assertEquals(AppControl.NotControlling, snapshot.phaseOf("11:22:33:44:55:66"))
    }
}
