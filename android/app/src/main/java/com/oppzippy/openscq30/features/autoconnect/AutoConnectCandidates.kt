package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.lib.wrapper.PairedDevice

internal object AutoConnectCandidates {
    fun resolve(
        connectedAddresses: List<String>,
        associatedAddresses: List<String>,
        savedDevices: List<PairedDevice>,
    ): List<String> {
        val associated = associatedAddresses.map { it.lowercase() }.toSet()
        val saved = savedDevices.filterNot { it.isDemo }.map { it.macAddress.lowercase() }.toSet()
        return connectedAddresses
            .distinctBy { it.lowercase() }
            .filter { it.lowercase() in associated && it.lowercase() in saved }
    }
}
