package com.oppzippy.openscq30.features.phonelink

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The stateful part of [AndroidPhoneLinkMonitor], kept free of Android types so the adapter wiring can be tested on
 * the JVM. It merges profile scans, classic-profile proxies and ACL broadcasts, and drops all evidence when Bluetooth
 * access is not ready.
 */
internal class PhoneLinkMonitorCore {
    private val state = MutableStateFlow(PhoneLinkSignals())
    val snapshot: StateFlow<PhoneLinkSignals> = state.asStateFlow()

    private val aclAddresses = mutableSetOf<String>()
    private val classicAddresses = mutableMapOf<Int, Set<String>>()
    private var gattAddresses: Set<String> = emptySet()
    private var access = BluetoothAccess.Unknown

    fun start() {
        clear()
        access = BluetoothAccess.Unknown
        publish()
    }

    fun stop() {
        clear()
        publish()
    }

    fun onAclConnected(address: String) {
        aclAddresses += canonicalMacAddress(address)
        publish()
    }

    fun onAclDisconnected(address: String) {
        aclAddresses -= canonicalMacAddress(address)
        publish()
    }

    fun onAdapterOff() {
        clear()
        access = BluetoothAccess.Off
        publish()
    }

    fun onAdapterOn() {
        clear()
        access = BluetoothAccess.Unknown
        publish()
    }

    fun applyScan(access: BluetoothAccess, gattAddresses: Collection<String>) {
        this.access = access
        if (access == BluetoothAccess.Ready) {
            this.gattAddresses = gattAddresses.mapTo(mutableSetOf(), ::canonicalMacAddress)
        } else {
            clear()
        }
        publish()
    }

    fun onProfileDevices(profile: Int, addresses: Collection<String>) {
        classicAddresses[profile] = addresses.mapTo(mutableSetOf(), ::canonicalMacAddress)
        publish()
    }

    fun onProfileDisconnected(profile: Int) {
        classicAddresses.remove(profile)
        publish()
    }

    private fun clear() {
        aclAddresses.clear()
        classicAddresses.clear()
        gattAddresses = emptySet()
    }

    private fun publish() {
        if (access != BluetoothAccess.Ready) {
            state.value = PhoneLinkSignals(access)
            return
        }
        val profileAddresses = buildSet {
            addAll(gattAddresses)
            classicAddresses.values.forEach { addAll(it) }
        }
        state.value = PhoneLinkSignals(
            access = access,
            profileAddresses = profileAddresses,
            aclAddresses = aclAddresses.toSet(),
        )
    }
}
