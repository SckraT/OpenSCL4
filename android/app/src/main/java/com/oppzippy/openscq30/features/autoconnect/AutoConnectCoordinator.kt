package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive

internal class AutoConnectCoordinator(
    private val isAutoConnectEnabled: () -> Boolean,
    private val associatedAddresses: () -> List<String>,
    private val savedDevices: suspend () -> List<PairedDevice>,
    private val connect: (String) -> Unit,
) {
    suspend fun handleConnectedDevice(macAddress: String) {
        if (!isAutoConnectEnabled()) return
        if (!isEligible(macAddress)) return
        if (!isAutoConnectEnabled()) return
        connect(macAddress)
    }

    suspend fun reconcileConnectedDevices(connectedAddresses: () -> List<String>) {
        if (!isAutoConnectEnabled()) return
        val candidates = AutoConnectCandidates.resolve(
            connectedAddresses = connectedAddresses(),
            associatedAddresses = associatedAddresses(),
            savedDevices = savedDevices(),
        )
        currentCoroutineContext().ensureActive()
        if (!isAutoConnectEnabled()) return
        candidates.forEach(connect)
    }

    private suspend fun isEligible(macAddress: String): Boolean = AutoConnectCandidates.resolve(
        connectedAddresses = listOf(macAddress),
        associatedAddresses = associatedAddresses(),
        savedDevices = savedDevices(),
    ).isNotEmpty()
}
