package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DeviceListLabelsTest {
    @Test
    fun everyRowStatusHasItsCombinedLine() {
        assertEquals(R.string.connected, DeviceListLabels.rowStatus(DeviceRowStatus.Connected))
        assertEquals(R.string.sound_device_connecting, DeviceListLabels.rowStatus(DeviceRowStatus.Connecting))
        assertEquals(
            R.string.sound_device_row_linked_without_control,
            DeviceListLabels.rowStatus(DeviceRowStatus.LinkedWithoutControl),
        )
        assertEquals(R.string.sound_device_not_connected, DeviceListLabels.rowStatus(DeviceRowStatus.NotConnected))
        assertEquals(
            R.string.sound_device_row_bluetooth_unknown,
            DeviceListLabels.rowStatus(DeviceRowStatus.BluetoothUnknown),
        )
        assertEquals(R.string.demo, DeviceListLabels.rowStatus(DeviceRowStatus.Demo))
    }

    @Test
    fun everyFeaturedBadgeHasItsLabel() {
        assertEquals(R.string.sound_device_phone_link_connected, DeviceListLabels.linkBadge(LinkBadge.Linked))
        assertEquals(R.string.sound_device_phone_link_not_connected, DeviceListLabels.linkBadge(LinkBadge.NotLinked))
        assertEquals(R.string.sound_device_phone_link_unknown, DeviceListLabels.linkBadge(LinkBadge.Unknown))
        assertEquals(R.string.sound_device_phone_link_unconfirmed, DeviceListLabels.linkBadge(LinkBadge.Unconfirmed))

        assertEquals(R.string.sound_device_control_active, DeviceListLabels.controlBadge(ControlBadge.Active))
        assertEquals(R.string.sound_device_connecting, DeviceListLabels.controlBadge(ControlBadge.Connecting))
        assertEquals(
            R.string.sound_device_control_unavailable,
            DeviceListLabels.controlBadge(ControlBadge.Unavailable),
        )
    }

    @Test
    fun everyBluetoothAccessHasItsBannerTitleAndAction() {
        assertNull(DeviceListLabels.accessTitle(BluetoothAccess.Ready))
        assertNull(DeviceListLabels.accessAction(BluetoothAccess.Ready))

        assertEquals(
            R.string.sound_permission_bluetooth_disabled,
            DeviceListLabels.accessTitle(BluetoothAccess.Off),
        )
        assertEquals(
            R.string.sound_permission_open_bluetooth_settings,
            DeviceListLabels.accessAction(BluetoothAccess.Off),
        )

        assertEquals(
            R.string.bluetooth_permission_is_required,
            DeviceListLabels.accessTitle(BluetoothAccess.PermissionDenied),
        )
        assertEquals(R.string.request_permission, DeviceListLabels.accessAction(BluetoothAccess.PermissionDenied))

        assertEquals(
            R.string.sound_device_bluetooth_unsupported,
            DeviceListLabels.accessTitle(BluetoothAccess.Unsupported),
        )
        assertNull(DeviceListLabels.accessAction(BluetoothAccess.Unsupported))

        assertEquals(
            R.string.sound_device_bluetooth_unknown,
            DeviceListLabels.accessTitle(BluetoothAccess.Unknown),
        )
        assertEquals(R.string.sound_device_bluetooth_retry, DeviceListLabels.accessAction(BluetoothAccess.Unknown))
    }

    @Test
    fun inaccessibleBluetoothNeverRendersTheNotConnectedWording() {
        for (access in BluetoothAccess.entries.filter { it != BluetoothAccess.Ready }) {
            val title = DeviceListLabels.accessTitle(access)
            assertNotEquals(R.string.sound_device_not_connected, title)
            assertNotEquals(R.string.sound_device_row_linked_without_control, title)
        }
        assertNotEquals(
            DeviceListLabels.rowStatus(DeviceRowStatus.NotConnected),
            DeviceListLabels.rowStatus(DeviceRowStatus.BluetoothUnknown),
        )
    }
}
