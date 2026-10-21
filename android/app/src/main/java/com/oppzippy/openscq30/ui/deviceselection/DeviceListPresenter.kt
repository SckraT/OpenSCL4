package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneBluetoothLink
import com.oppzippy.openscq30.features.phonelink.PhoneLinkReducer
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSignals
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSnapshot
import com.oppzippy.openscq30.features.phonelink.canonicalMacAddress
import com.oppzippy.openscq30.lib.wrapper.PairedDevice

object DeviceListPresenter {
    fun present(
        devices: List<PairedDevice>,
        phoneLink: PhoneLinkSignals,
        appControl: AppControlSnapshot,
        lastDeviceMacAddress: String? = null,
        isLoading: Boolean = false,
    ): DeviceListState {
        val links = PhoneLinkReducer.reduce(
            access = phoneLink.access,
            profileAddresses = phoneLink.profileAddresses,
            aclAddresses = phoneLink.aclAddresses,
            controlledAddress = appControl.macAddress.takeIf { appControl.phase == AppControl.Controlling },
        )
        val featuredDevice = featuredDevice(devices, links, appControl, lastDeviceMacAddress)
        return DeviceListState(
            access = links.access,
            featured = featuredDevice?.let { rowState(it, links, appControl) },
            others = devices.filterNot { it == featuredDevice }.map { rowState(it, links, appControl) },
            isLoading = isLoading,
        )
    }

    private fun rowState(
        device: PairedDevice,
        links: PhoneLinkSnapshot,
        appControl: AppControlSnapshot,
    ): DeviceRowState {
        if (device.isDemo) {
            return DeviceRowState(
                device = device,
                link = PhoneBluetoothLink.Unknown,
                control = AppControl.NotControlling,
                status = DeviceRowStatus.Demo,
                linkBadge = LinkBadge.Unknown,
                controlBadge = ControlBadge.Unavailable,
                tapAction = TapAction.Open,
            )
        }

        val control = appControl.phaseOf(device.macAddress)
        val link = links.linkOf(device.macAddress)
        return DeviceRowState(
            device = device,
            link = link,
            control = control,
            status = statusOf(control, link),
            linkBadge = linkBadgeOf(link, control),
            controlBadge = controlBadgeOf(control),
            tapAction = tapActionOf(control, links.access),
        )
    }

    private fun statusOf(control: AppControl, link: PhoneBluetoothLink): DeviceRowStatus = when {
        control == AppControl.Controlling -> DeviceRowStatus.Connected
        control == AppControl.Connecting -> DeviceRowStatus.Connecting
        link == PhoneBluetoothLink.Linked -> DeviceRowStatus.LinkedWithoutControl
        link == PhoneBluetoothLink.NotLinked -> DeviceRowStatus.NotConnected
        else -> DeviceRowStatus.BluetoothUnknown
    }

    private fun linkBadgeOf(link: PhoneBluetoothLink, control: AppControl): LinkBadge = when {
        link == PhoneBluetoothLink.Linked -> LinkBadge.Linked
        link == PhoneBluetoothLink.NotLinked -> LinkBadge.NotLinked
        control == AppControl.Controlling -> LinkBadge.Unconfirmed
        else -> LinkBadge.Unknown
    }

    private fun controlBadgeOf(control: AppControl): ControlBadge = when (control) {
        AppControl.Controlling -> ControlBadge.Active
        AppControl.Connecting -> ControlBadge.Connecting
        AppControl.NotControlling -> ControlBadge.Unavailable
    }

    private fun tapActionOf(control: AppControl, access: BluetoothAccess): TapAction = when {
        control == AppControl.Controlling -> TapAction.Open
        control == AppControl.Connecting -> TapAction.None
        access != BluetoothAccess.Ready -> TapAction.ResolveBluetoothAccess
        else -> TapAction.Connect
    }

    private fun featuredDevice(
        devices: List<PairedDevice>,
        links: PhoneLinkSnapshot,
        appControl: AppControlSnapshot,
        lastDeviceMacAddress: String?,
    ): PairedDevice? {
        devices.firstOrNull { appControl.phaseOf(it.macAddress) != AppControl.NotControlling }?.let { return it }

        val linked = devices.filter {
            !it.isDemo && links.linkOf(it.macAddress) == PhoneBluetoothLink.Linked
        }
        linked.firstOrNull { sameMacAddress(it.macAddress, lastDeviceMacAddress) }?.let { return it }
        linked.firstOrNull()?.let { return it }

        devices.firstOrNull { sameMacAddress(it.macAddress, lastDeviceMacAddress) }?.let { return it }
        return devices.firstOrNull()
    }

    private fun AppControlSnapshot.phaseOf(macAddress: String): AppControl {
        val controlledMacAddress = this.macAddress ?: return AppControl.NotControlling
        return if (canonicalMacAddress(controlledMacAddress) == canonicalMacAddress(macAddress)) {
            phase
        } else {
            AppControl.NotControlling
        }
    }

    private fun sameMacAddress(left: String, right: String?): Boolean =
        right != null && canonicalMacAddress(left) == canonicalMacAddress(right)
}
