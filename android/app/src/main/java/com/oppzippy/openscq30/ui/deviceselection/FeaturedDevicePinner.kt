package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.phonelink.canonicalMacAddress
import com.oppzippy.openscq30.lib.wrapper.PairedDevice

/**
 * Keeps the featured device from jumping between rows while the list is open. The link set changes on every ACL
 * event; only the saved devices, the app-control axis or the last-used device may choose a new featured device.
 */
class FeaturedDevicePinner {
    private var pinnedKey: FeaturedPinKey? = null
    private var pinnedMacAddress: String? = null

    fun reset() {
        pinnedKey = null
        pinnedMacAddress = null
    }

    fun pinnedFeaturedMacAddress(
        devices: List<PairedDevice>,
        control: AppControlSnapshot,
        lastDeviceMacAddress: String?,
    ): String? {
        val key = key(devices, control, lastDeviceMacAddress)
        return if (key == pinnedKey) pinnedMacAddress else null
    }

    fun remember(
        state: DeviceListState,
        devices: List<PairedDevice>,
        control: AppControlSnapshot,
        lastDeviceMacAddress: String?,
    ) {
        pinnedKey = key(devices, control, lastDeviceMacAddress)
        pinnedMacAddress = state.featured?.device?.macAddress
    }

    private fun key(
        devices: List<PairedDevice>,
        control: AppControlSnapshot,
        lastDeviceMacAddress: String?,
    ): FeaturedPinKey = FeaturedPinKey(
        devices = devices.map { canonicalMacAddress(it.macAddress) },
        controlMacAddress = control.macAddress?.let(::canonicalMacAddress),
        controlPhase = control.phase,
        lastDeviceMacAddress = lastDeviceMacAddress?.let(::canonicalMacAddress),
    )

    private data class FeaturedPinKey(
        val devices: List<String>,
        val controlMacAddress: String?,
        val controlPhase: AppControl,
        val lastDeviceMacAddress: String?,
    )
}
