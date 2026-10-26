package com.oppzippy.openscq30.features.autoconnect

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AutoConnectForegroundTest {
    @Test
    fun holdsTheNotificationWhileEnabledAndNotConnected() {
        assertTrue(shouldHoldForeground(autoConnectEnabled = true, deviceConnected = false))
    }

    @Test
    fun dropsTheNotificationOnceTheDeviceIsConnected() {
        assertFalse(shouldHoldForeground(autoConnectEnabled = true, deviceConnected = true))
    }

    @Test
    fun holdsNoNotificationWhenAutoconnectIsDisabled() {
        assertFalse(shouldHoldForeground(autoConnectEnabled = false, deviceConnected = false))
        assertFalse(shouldHoldForeground(autoConnectEnabled = false, deviceConnected = true))
    }

    @Test
    fun notificationPermissionIsRequiredFromAndroid13() {
        assertTrue(canEnableAutoConnect(sdkInt = 33, notificationsGranted = true))
        assertFalse(canEnableAutoConnect(sdkInt = 33, notificationsGranted = false))
    }

    @Test
    fun olderAndroidNeedsNoNotificationPermission() {
        // Before Android 13 a foreground notification needs no runtime permission.
        assertTrue(canEnableAutoConnect(sdkInt = 32, notificationsGranted = false))
        assertTrue(canEnableAutoConnect(sdkInt = 26, notificationsGranted = false))
    }
}
