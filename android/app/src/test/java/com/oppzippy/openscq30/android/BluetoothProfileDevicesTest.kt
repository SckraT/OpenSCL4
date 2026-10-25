package com.oppzippy.openscq30.android

import android.bluetooth.BluetoothProfile
import org.junit.Assert.assertEquals
import org.junit.Test

class BluetoothProfileDevicesTest {
    private val liberty = "F4:9D:8A:7C:99:06"
    private val bluetooth = FakeBluetoothProfiles()
    private val reports = mutableListOf<Pair<Int, List<String>>>()
    private val disconnectedProfiles = mutableListOf<Int>()
    private val profileDevices = BluetoothProfileDevices(
        bluetooth = bluetooth,
        onProfileDevices = { profile, addresses -> reports += profile to addresses },
        onProfileDisconnected = { disconnectedProfiles += it },
    )

    @Test
    fun rereadReportsTheDevicesConnectedNow() {
        profileDevices.open()
        bluetooth.connectProxies()
        reports.clear()
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)

        profileDevices.reread()

        assertEquals(
            listOf(BluetoothProfile.A2DP to listOf(liberty), BluetoothProfile.HEADSET to emptyList()),
            reports,
        )
    }

    @Test
    fun aProxyThatAndroidDropsReportsItsProfileDisconnectedAndIsNotReread() {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)
        profileDevices.open()
        bluetooth.connectProxies()

        bluetooth.dropProxies()
        reports.clear()
        profileDevices.reread()

        assertEquals(listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET), disconnectedProfiles)
        assertEquals(emptyList<Pair<Int, List<String>>>(), reports)
    }

    @Test
    fun openingAgainKeepsPendingAndConnectedProxies() {
        profileDevices.open()
        profileDevices.open()
        bluetooth.connectProxies()
        profileDevices.open()
        bluetooth.connectProxies()

        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.proxies.map { it.request.profile },
        )
    }

    @Test
    fun openingAgainReplacesProxiesThatAndroidDropped() {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)
        profileDevices.open()
        bluetooth.connectProxies()
        bluetooth.dropProxies()
        reports.clear()

        profileDevices.open()
        bluetooth.connectProxies()

        assertEquals(
            listOf(BluetoothProfile.A2DP to listOf(liberty), BluetoothProfile.HEADSET to emptyList()),
            reports,
        )
        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.openProxies.map { it.request.profile },
        )
    }

    @Test
    fun aDroppedProxyIsNotRequestedAgainBeforeOpeningAgain() {
        profileDevices.open()
        bluetooth.connectProxies()

        bluetooth.dropProxies()
        bluetooth.connectProxies()

        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.proxies.map { it.request.profile },
        )
    }

    @Test
    fun openingAgainKeepsProxiesThatAndroidRestored() {
        profileDevices.open()
        bluetooth.connectProxies()
        bluetooth.dropProxies()
        bluetooth.restoreProxies()

        profileDevices.open()
        bluetooth.connectProxies()

        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.proxies.map { it.request.profile },
        )
    }

    @Test
    fun gattDevicesComeFromBothGattProfiles() {
        bluetooth.devices[BluetoothProfile.GATT] = listOf(liberty)
        bluetooth.devices[BluetoothProfile.GATT_SERVER] = listOf("AC:12:2F:C8:6E:08")

        assertEquals(listOf(liberty, "AC:12:2F:C8:6E:08"), profileDevices.gattDevices())
    }

    @Test
    fun gattDevicesAreUnknownWhenNeitherGattProfileCanBeRead() {
        bluetooth.unsupportedProfiles += listOf(BluetoothProfile.GATT, BluetoothProfile.GATT_SERVER)

        assertEquals(null, profileDevices.gattDevices())
    }
}
