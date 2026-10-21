package com.oppzippy.openscq30.ui.deviceselection

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
}
