package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneBluetoothLink
import com.oppzippy.openscq30.features.phonelink.canonicalMacAddress
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.lib.wrapper.PairedDevice

/** Whether OpenSCL4 currently controls a device. */
enum class AppControl { Controlling, Connecting, NotControlling }

/** The app-control axis, derived from the connection status. */
data class AppControlSnapshot(val macAddress: String? = null, val phase: AppControl = AppControl.NotControlling)

/** The single mapping from the service's connection state to the app-control axis. */
fun ConnectionStatus.toAppControlSnapshot(): AppControlSnapshot = when (this) {
    is ConnectionStatus.Connected -> AppControlSnapshot(macAddress, AppControl.Controlling)

    is ConnectionStatus.Connecting -> AppControlSnapshot(macAddress, AppControl.Connecting)

    is ConnectionStatus.Recovering -> AppControlSnapshot(macAddress, AppControl.Connecting)

    // Keep the device screen open so it can show the lost state with Retry.
    is ConnectionStatus.Lost -> AppControlSnapshot(macAddress, AppControl.Controlling)

    ConnectionStatus.AwaitingConnection, ConnectionStatus.Disconnected -> AppControlSnapshot()
}

/** The phase for one device, or [AppControl.NotControlling] when another device is controlled. */
fun AppControlSnapshot.phaseOf(macAddress: String): AppControl {
    val controlledMacAddress = this.macAddress ?: return AppControl.NotControlling
    return if (canonicalMacAddress(controlledMacAddress) == canonicalMacAddress(macAddress)) {
        phase
    } else {
        AppControl.NotControlling
    }
}

/** The combined line shown on a row that is not the featured card. */
enum class DeviceRowStatus { Connected, Connecting, LinkedWithoutControl, NotConnected, BluetoothUnknown, Demo }

/** The phone-link badge on the featured card. */
enum class LinkBadge { Linked, NotLinked, Unknown, Unconfirmed }

/** The app-control badge on the featured card. */
enum class ControlBadge { Active, Connecting, Unavailable }

/** The action a tap on a device should take. */
enum class TapAction { Open, Connect, None, ResolveBluetoothAccess }

data class DeviceRowState(
    val device: PairedDevice,
    val link: PhoneBluetoothLink,
    val control: AppControl,
    val status: DeviceRowStatus,
    val linkBadge: LinkBadge,
    val controlBadge: ControlBadge,
    val tapAction: TapAction,
)

data class DeviceListState(
    val access: BluetoothAccess,
    val featured: DeviceRowState?,
    val others: List<DeviceRowState>,
    val isLoading: Boolean = false,
)
