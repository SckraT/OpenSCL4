package com.oppzippy.openscq30.features.autoconnect

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AutoConnectForegroundTest {
    @Test
    fun holdsTheNotificationWhileEnabledAndNoDeviceServiceRuns() {
        assertTrue(shouldHoldForeground(autoConnectEnabled = true, deviceServiceRunning = false))
    }

    @Test
    fun dropsTheNotificationOnceTheDeviceServiceRuns() {
        // That service is already foreground, so a second permanent notification would only be noise.
        assertFalse(shouldHoldForeground(autoConnectEnabled = true, deviceServiceRunning = true))
    }

    @Test
    fun holdsNoNotificationWhenAutoconnectIsDisabled() {
        assertFalse(shouldHoldForeground(autoConnectEnabled = false, deviceServiceRunning = false))
        assertFalse(shouldHoldForeground(autoConnectEnabled = false, deviceServiceRunning = true))
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
