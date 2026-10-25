package com.oppzippy.openscq30.android

import android.bluetooth.BluetoothProfile

/**
 * The profile scan of `docs/android-connection-spec.md` §3.2. Classic A2DP and HEADSET are read through profile
 * proxies: on Android 16 `BluetoothManager.getConnectedDevices` throws `IllegalArgumentException: Profile not
 * supported` for them, so it is used only for GATT and GATT_SERVER. Addresses are reported as Android formats them;
 * compare them case-insensitively.
 *
 * Without the Bluetooth permissions a classic profile reports nothing, so [open] and [reread] need no permission
 * check first; [gattDevices] throws [SecurityException] instead, which the caller turns into its own state.
 *
 * Calls and [bluetooth] callbacks are expected on the main thread.
 */
internal class BluetoothProfileDevices(
    private val bluetooth: BluetoothProfiles,
    private val onProfileDevices: (profile: Int, addresses: List<String>) -> Unit,
    private val onProfileDisconnected: (profile: Int) -> Unit = {},
) {
    private val requests = mutableMapOf<Int, ProxyRequest>()

    /**
     * Requests a proxy for each classic profile; a profile reports its devices once its proxy connects. Opening again
     * keeps pending and connected proxies and replaces the ones Android dropped, for example after the adapter
     * turned off.
     */
    fun open() {
        for (profile in CLASSIC_PROFILES) {
            val current = requests[profile]
            if (current != null && !current.isDropped) continue
            current?.close()
            val request = ProxyRequest(profile)
            requests[profile] = request
            if (!request.open()) requests.remove(profile)
        }
    }

    /** Reports the devices of every connected proxy again. */
    fun reread() {
        requests.values.toList().forEach { it.report() }
    }

    /** Closes every proxy, including ones that connect later; they report nothing. */
    fun close() {
        val closing = requests.values.toList()
        requests.clear()
        closing.forEach { it.close() }
    }

    /** Closes the proxy of [profile], so Android cannot restore it and report again; [open] requests it anew. */
    fun close(profile: Int) {
        requests.remove(profile)?.close()
    }

    /**
     * Devices connected over GATT or GATT_SERVER, or null when neither profile can be read. A missing Bluetooth
     * permission throws [SecurityException].
     */
    fun gattDevices(): List<String>? {
        var readable = false
        val addresses = mutableListOf<String>()
        for (profile in GATT_PROFILES) {
            try {
                addresses += bluetooth.connectedDevices(profile)
                readable = true
            } catch (_: IllegalArgumentException) {
            }
        }
        return if (readable) addresses else null
    }

    private inner class ProxyRequest(val profile: Int) {
        // A dropped proxy is kept so that close() still releases it; Android may restore it.
        private var proxy: ProfileProxy? = null
        private var isConnected = false
        private var isClosed = false
        val isDropped: Boolean get() = proxy != null && !isConnected

        fun open(): Boolean = try {
            bluetooth.openProxy(profile, onConnected = ::onConnected, onDisconnected = ::onDisconnected)
        } catch (_: SecurityException) {
            false
        }

        private fun onConnected(proxy: ProfileProxy) {
            // Android cannot cancel a pending request, so a proxy that arrives after close is closed here.
            if (isClosed) {
                proxy.close()
                return
            }
            this.proxy = proxy
            isConnected = true
            report()
        }

        private fun onDisconnected() {
            if (isClosed) return
            isConnected = false
            onProfileDisconnected(profile)
        }

        fun report() {
            val proxy = proxy?.takeIf { isConnected } ?: return
            val addresses = try {
                proxy.connectedDevices()
            } catch (_: SecurityException) {
                return
            }
            onProfileDevices(profile, addresses)
        }

        fun close() {
            isClosed = true
            proxy?.close()
        }
    }

    private companion object {
        val CLASSIC_PROFILES = listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET)
        val GATT_PROFILES = listOf(BluetoothProfile.GATT, BluetoothProfile.GATT_SERVER)
    }
}

/**
 * The Android Bluetooth calls behind [BluetoothProfileDevices]; [AndroidBluetoothProfiles] implements them. Like
 * Android's, they throw [SecurityException] without the Bluetooth permissions.
 */
internal interface BluetoothProfiles {
    /**
     * `BluetoothAdapter.getProfileProxy`; false when the proxy cannot be requested. [onConnected] runs when the proxy
     * is ready, and again when Android restores it; [onDisconnected] runs when Android drops it.
     */
    fun openProxy(profile: Int, onConnected: (ProfileProxy) -> Unit, onDisconnected: () -> Unit): Boolean

    /** `BluetoothManager.getConnectedDevices`; throws [IllegalArgumentException] for an unsupported profile. */
    fun connectedDevices(profile: Int): List<String>
}

internal interface ProfileProxy {
    /** `BluetoothProfile.getConnectedDevices`; throws [SecurityException] without the Bluetooth permissions. */
    fun connectedDevices(): List<String>

    /** `BluetoothAdapter.closeProfileProxy`. */
    fun close()
}
