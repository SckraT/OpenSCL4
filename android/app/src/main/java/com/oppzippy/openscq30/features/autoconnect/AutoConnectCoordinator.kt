package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.features.phonelink.canonicalMacAddress
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive

internal class AutoConnectCoordinator(
    private val isAutoConnectEnabled: () -> Boolean,
    private val associatedAddresses: () -> List<String>,
    private val savedDevices: suspend () -> List<PairedDevice>,
    private val connect: (String) -> Unit,
) {
    private val reconciledAddresses = mutableSetOf<String>()

    suspend fun handleConnectedDevice(macAddress: String) {
        if (!isAutoConnectEnabled()) return
        if (!isEligible(macAddress)) return
        if (!isAutoConnectEnabled()) return
        connect(macAddress)
    }

    /**
     * Connects devices that were already connected when autoconnect started. Each profile reports separately, so a
     * device is connected at most once however many reports list it.
     */
    suspend fun reconcileConnectedDevices(connectedAddresses: List<String>) {
        if (!isAutoConnectEnabled()) return
        val candidates = AutoConnectCandidates.resolve(
            connectedAddresses = connectedAddresses,
            associatedAddresses = associatedAddresses(),
            savedDevices = savedDevices(),
        )
        currentCoroutineContext().ensureActive()
        if (!isAutoConnectEnabled()) return
        for (address in candidates) {
            if (canonicalMacAddress(address) in reconciledAddresses) continue
            connect(address)
            // Marked only once started: a failed start leaves the device to the next report.
            reconciledAddresses += canonicalMacAddress(address)
        }
    }

    private suspend fun isEligible(macAddress: String): Boolean = AutoConnectCandidates.resolve(
        connectedAddresses = listOf(macAddress),
        associatedAddresses = associatedAddresses(),
        savedDevices = savedDevices(),
    ).isNotEmpty()
}
