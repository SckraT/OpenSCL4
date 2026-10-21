package com.oppzippy.openscq30.ui.deviceselection

import androidx.annotation.StringRes
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.phonelink.BluetoothAccess

/** Text for the two status axes, the combined row line and the global Bluetooth access banner. */
object DeviceListLabels {
    @StringRes
    fun rowStatus(status: DeviceRowStatus): Int = when (status) {
        DeviceRowStatus.Connected -> R.string.connected
        DeviceRowStatus.Connecting -> R.string.sound_device_connecting
        DeviceRowStatus.LinkedWithoutControl -> R.string.sound_device_row_linked_without_control
        DeviceRowStatus.NotConnected -> R.string.sound_device_not_connected
        DeviceRowStatus.BluetoothUnknown -> R.string.sound_device_row_bluetooth_unknown
        DeviceRowStatus.Demo -> R.string.demo
    }

    @StringRes
    fun linkBadge(badge: LinkBadge): Int = when (badge) {
        LinkBadge.Linked -> R.string.sound_device_phone_link_connected
        LinkBadge.NotLinked -> R.string.sound_device_phone_link_not_connected
        LinkBadge.Unknown -> R.string.sound_device_phone_link_unknown
        LinkBadge.Unconfirmed -> R.string.sound_device_phone_link_unconfirmed
    }

    @StringRes
    fun controlBadge(badge: ControlBadge): Int = when (badge) {
        ControlBadge.Active -> R.string.sound_device_control_active
        ControlBadge.Connecting -> R.string.sound_device_connecting
        ControlBadge.Unavailable -> R.string.sound_device_control_unavailable
    }

    @StringRes
    fun accessTitle(access: BluetoothAccess): Int? = when (access) {
        BluetoothAccess.Ready -> null
        BluetoothAccess.Off -> R.string.sound_permission_bluetooth_disabled
        BluetoothAccess.PermissionDenied -> R.string.bluetooth_permission_is_required
        BluetoothAccess.Unsupported -> R.string.sound_device_bluetooth_unsupported
        BluetoothAccess.Unknown -> R.string.sound_device_bluetooth_unknown
    }

    @StringRes
    fun accessHint(access: BluetoothAccess): Int? = when (access) {
        BluetoothAccess.Ready -> null
        BluetoothAccess.Off -> R.string.sound_permission_bluetooth_disabled_hint
        BluetoothAccess.PermissionDenied -> R.string.sound_permission_bluetooth_purpose
        BluetoothAccess.Unsupported -> R.string.sound_device_bluetooth_unsupported_hint
        BluetoothAccess.Unknown -> R.string.sound_device_bluetooth_unknown_hint
    }

    @StringRes
    fun accessAction(access: BluetoothAccess): Int? = when (access) {
        BluetoothAccess.Ready, BluetoothAccess.Unsupported -> null
        BluetoothAccess.Off -> R.string.sound_permission_open_bluetooth_settings
        BluetoothAccess.PermissionDenied -> R.string.request_permission
        BluetoothAccess.Unknown -> R.string.sound_device_bluetooth_retry
    }
}
