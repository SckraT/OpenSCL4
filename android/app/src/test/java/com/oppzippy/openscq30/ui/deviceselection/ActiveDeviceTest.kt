package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Setting
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ActiveDeviceTest {
    private val liberty = PairedDevice("F4:9D:8A:7C:99:06", "SoundcoreA3954", false)
    private val q30 = PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3028", false)
    private val devices = listOf(q30, liberty)

    @Test
    fun connectedDeviceIsFeaturedEvenIfAnotherWasUsedLast() {
        assertEquals(liberty, featuredDevice(devices, "f4:9d:8a:7c:99:06", q30.macAddress))
    }

    @Test
    fun lastUsedDeviceIsFeaturedWhenNothingIsConnected() {
        assertEquals(liberty, featuredDevice(devices, null, liberty.macAddress))
    }

    @Test
    fun firstDeviceIsFeaturedWithoutHistory() {
        assertEquals(q30, featuredDevice(devices, null, null))
        assertEquals(q30, featuredDevice(devices, null, "00:00:00:00:00:00"))
    }

    @Test
    fun nothingIsFeaturedWithoutDevices() {
        assertNull(featuredDevice(emptyList(), liberty.macAddress, liberty.macAddress))
    }

    @Test
    fun brandIsSplitFromTheModelName() {
        assertEquals("Soundcore" to "Liberty 4 Pro", splitBrand("Soundcore Liberty 4 Pro"))
        assertEquals(null to "Development Device", splitBrand("Development Device"))
        assertEquals(null to "Soundcore", splitBrand("Soundcore"))
    }

    @Test
    fun batteriesAreReadInOverviewOrder() {
        val settings = mapOf(
            "caseBatteryLevel" to Setting.InformationSetting("8/10", "80%"),
            "batteryLevelLeft" to Setting.InformationSetting("77/100", "77%"),
            "batteryLevelRight" to Setting.InformationSetting("66/100", "66%"),
        )
        assertEquals(
            listOf(
                DeviceBattery(R.string.sound_overview_left, "77%", 0.77f),
                DeviceBattery(R.string.sound_overview_right, "66%", 0.66f),
                DeviceBattery(R.string.sound_overview_case, "80%", 0.8f),
            ),
            deviceBatteries(settings),
        )
    }
}
