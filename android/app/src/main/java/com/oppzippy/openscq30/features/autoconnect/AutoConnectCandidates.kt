package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.features.phonelink.canonicalMacAddress
import com.oppzippy.openscq30.lib.wrapper.PairedDevice

internal object AutoConnectCandidates {
    fun resolve(
        connectedAddresses: List<String>,
        associatedAddresses: List<String>,
        savedDevices: List<PairedDevice>,
    ): List<String> {
        val associated = associatedAddresses.map(::canonicalMacAddress).toSet()
        val saved = savedDevices.filterNot { it.isDemo }.map { canonicalMacAddress(it.macAddress) }.toSet()
        return connectedAddresses
            .distinctBy(::canonicalMacAddress)
            .filter { canonicalMacAddress(it) in associated && canonicalMacAddress(it) in saved }
    }
}
