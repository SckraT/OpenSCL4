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
    // Devices this coordinator has started, by canonical address; the start reconcile skips them.
    private val startedAddresses = mutableSetOf<String>()

    /** Connects a device on its ACL broadcast, also when it was started before: the broadcast means it reconnected. */
    suspend fun handleConnectedDevice(macAddress: String) {
        if (!isAutoConnectEnabled()) return
        if (!isEligible(macAddress)) return
        if (!isAutoConnectEnabled()) return
        start(macAddress)
    }

    /**
     * Connects devices that were already connected when autoconnect started. Each profile reports separately and a
     * device connecting meanwhile is started by its broadcast, so a device is connected at most once however many
     * reports list it.
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
        candidates.filterNot { canonicalMacAddress(it) in startedAddresses }.forEach(::start)
    }

    private fun start(macAddress: String) {
        connect(macAddress)
        // Marked only once started: a failed start leaves the device to the next report.
        startedAddresses += canonicalMacAddress(macAddress)
    }

    private suspend fun isEligible(macAddress: String): Boolean = AutoConnectCandidates.resolve(
        connectedAddresses = listOf(macAddress),
        associatedAddresses = associatedAddresses(),
        savedDevices = savedDevices(),
    ).isNotEmpty()
}
