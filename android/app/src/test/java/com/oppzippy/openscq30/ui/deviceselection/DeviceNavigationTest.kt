package com.oppzippy.openscq30.ui.deviceselection

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceNavigationTest {
    private val mac = "AA:BB:CC:DD:EE:FF"
    private val otherMac = "11:22:33:44:55:66"

    @Test
    fun openingADeviceShowsTheDeviceScreen() {
        val state = DeviceNavigationState().openDevice(mac)

        assertTrue(state.showDevice)
        assertEquals(mac, state.requestedMacAddress)
    }

    @Test
    fun requestingControlKeepsTheListUntilTheDeviceIsControlled() {
        var state = DeviceNavigationState().requestControl(mac)
        state = state.onAppControl(AppControlSnapshot(mac, AppControl.Connecting))

        assertFalse(state.showDevice)
        assertTrue(state.showDeviceList)
        assertEquals(mac, state.listConnectMacAddress)

        state = state.onAppControl(AppControlSnapshot(mac, AppControl.Controlling))

        assertTrue(state.showDevice)
        assertEquals(mac, state.requestedMacAddress)
        assertNull(state.listConnectMacAddress)
    }

    @Test
    fun aFailedConnectionKeepsTheListVisibleAndAllowsAnotherTry() {
        var state = DeviceNavigationState().requestControl(mac)
        state = state.onAppControl(AppControlSnapshot(mac, AppControl.NotControlling))

        assertFalse(state.showDevice)
        assertTrue(state.showDeviceList)
        assertNull(state.listConnectMacAddress)

        state = state.requestControl(mac)
        assertEquals(mac, state.listConnectMacAddress)
        assertFalse(state.showDevice)
    }

    @Test
    fun anotherDeviceBeingControlledDoesNotOpenTheListConnectTarget() {
        var state = DeviceNavigationState().requestControl(mac)
        state = state.onAppControl(AppControlSnapshot(otherMac, AppControl.Controlling))

        assertFalse(state.showDevice)
        assertEquals(mac, state.listConnectMacAddress)
    }

    @Test
    fun autoConnectStillOpensTheConnectingScreen() {
        val state = DeviceNavigationState().onAppControl(AppControlSnapshot(mac, AppControl.Connecting))

        assertTrue(state.showDevice)
        assertEquals(mac, state.requestedMacAddress)
    }

    @Test
    fun aConnectedDeviceDoesNotPullTheUserOffTheListOnItsOwn() {
        val opened = DeviceNavigationState().openDevice(mac).showList()
        val reconnected = opened.onAppControl(AppControlSnapshot(mac, AppControl.Controlling))

        assertFalse(reconnected.showDevice)
        assertEquals(mac, reconnected.requestedMacAddress)
    }

    @Test
    fun goingBackShowsTheListWithoutForgettingTheOpenDevice() {
        val state = DeviceNavigationState().openDevice(mac).showList()

        assertFalse(state.showDevice)
        assertEquals(mac, state.requestedMacAddress)
    }

    @Test
    fun unpairedDevicesAreForgotten() {
        val state = DeviceNavigationState()
            .requestControl(mac)
            .forgetDevice(mac)

        assertNull(state.listConnectMacAddress)
        assertNull(state.requestedMacAddress)
    }

    @Test
    fun unparingAnotherDevicePreservesTheRequestedDevice() {
        val state = DeviceNavigationState()
            .openDevice(mac)
            .forgetDevice(otherMac)

        assertEquals(mac, state.requestedMacAddress)
    }
}
