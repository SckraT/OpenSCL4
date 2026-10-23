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

    fun accessPresentation(access: BluetoothAccess): BluetoothAccessPresentation = when (access) {
        BluetoothAccess.Ready -> BluetoothAccessPresentation()

        BluetoothAccess.Off -> BluetoothAccessPresentation(
            title = R.string.sound_permission_bluetooth_disabled,
            hint = R.string.sound_permission_bluetooth_disabled_hint,
            action = R.string.sound_permission_open_bluetooth_settings,
        )

        BluetoothAccess.PermissionDenied -> BluetoothAccessPresentation(
            title = R.string.bluetooth_permission_is_required,
            hint = R.string.sound_permission_bluetooth_purpose,
            action = R.string.request_permission,
        )

        BluetoothAccess.Unsupported -> BluetoothAccessPresentation(
            title = R.string.sound_device_bluetooth_unsupported,
            hint = R.string.sound_device_bluetooth_unsupported_hint,
        )

        BluetoothAccess.Unknown -> BluetoothAccessPresentation(
            title = R.string.sound_device_bluetooth_unknown,
            hint = R.string.sound_device_bluetooth_unknown_hint,
            action = R.string.sound_device_bluetooth_retry,
        )
    }

    @StringRes
    fun accessTitle(access: BluetoothAccess): Int? = accessPresentation(access).title

    @StringRes
    fun accessHint(access: BluetoothAccess): Int? = accessPresentation(access).hint

    @StringRes
    fun accessAction(access: BluetoothAccess): Int? = accessPresentation(access).action
}

data class BluetoothAccessPresentation(
    @StringRes val title: Int? = null,
    @StringRes val hint: Int? = null,
    @StringRes val action: Int? = null,
)
