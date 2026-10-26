package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import org.junit.Assert.assertNotEquals
import org.junit.Test

class AutoConnectNotificationIdTest {
    @Test
    fun usesItsOwnNotificationId() {
        // Notification ids are shared by the whole app, so a collision would let the autoconnect notification
        // overwrite or remove the device service notification, and the other way round.
        assertNotEquals(DeviceService.NOTIFICATION_ID, AutoConnectService.NOTIFICATION_ID)
    }

    @Test
    fun usesItsOwnNotificationChannel() {
        assertNotEquals(DeviceService.NOTIFICATION_CHANNEL_ID, AutoConnectService.NOTIFICATION_CHANNEL_ID)
    }
}
