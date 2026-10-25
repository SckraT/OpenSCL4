package com.oppzippy.openscq30.android

import android.bluetooth.BluetoothProfile

/**
 * Bluetooth as Android 16 behaves on a Pixel 8 Pro: `BluetoothManager.getConnectedDevices` rejects A2DP and HEADSET,
 * profile proxies arrive asynchronously, and closing a proxy reports it disconnected to its listener.
 */
internal class FakeBluetoothProfiles : BluetoothProfiles {
    /** Devices each profile reports, with addresses formatted as Android formats them. */
    val devices = mutableMapOf<Int, List<String>>()

    /** Profiles `BluetoothManager.getConnectedDevices` rejects. */
    val unsupportedProfiles = mutableSetOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET)
    val proxies = mutableListOf<FakeProxy>()
    private val requests = mutableListOf<Request>()

    val openProxies: List<FakeProxy> get() = proxies.filterNot { it.isClosed }

    override fun openProxy(profile: Int, onConnected: (ProfileProxy) -> Unit, onDisconnected: () -> Unit): Boolean {
        requests += Request(profile, onConnected, onDisconnected)
        return true
    }

    override fun connectedDevices(profile: Int): List<String> {
        require(profile !in unsupportedProfiles) { "Profile not supported: $profile" }
        return devices[profile].orEmpty()
    }

    /** Delivers every requested proxy, as Android does once the profile service is bound. */
    fun connectProxies() {
        val pending = requests.toList()
        requests.clear()
        for (request in pending) {
            val proxy = FakeProxy(request)
            proxies += proxy
            request.onConnected(proxy)
        }
    }

    /** Drops every open proxy, as Android does when the adapter turns off. */
    fun dropProxies() {
        openProxies.forEach { it.request.onDisconnected() }
    }

    class Request(val profile: Int, val onConnected: (ProfileProxy) -> Unit, val onDisconnected: () -> Unit)

    inner class FakeProxy(val request: Request) : ProfileProxy {
        var isClosed = false
            private set

        override fun connectedDevices(): List<String> = devices[request.profile].orEmpty()

        override fun close() {
            if (isClosed) return
            isClosed = true
            request.onDisconnected()
        }
    }
}
