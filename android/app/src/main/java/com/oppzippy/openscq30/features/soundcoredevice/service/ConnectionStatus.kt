package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.ClientSettingAvailability
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason

/** What the LDAC Quick Settings tile shows: the device's current value and whether it can be changed. */
data class LdacTileState(val enabled: Boolean, val availability: ClientSettingAvailability)

sealed class ConnectionStatus {
    abstract val macAddress: String?
    abstract val deviceManager: DeviceConnectionManager?

    data object AwaitingConnection : ConnectionStatus() {
        override val macAddress: String? = null
        override val deviceManager: DeviceConnectionManager? = null
    }

    class Connecting(override val macAddress: String) : ConnectionStatus() {
        override val deviceManager: DeviceConnectionManager? = null
    }

    class Connected(override val deviceManager: DeviceConnectionManager, override val macAddress: String? = null) :
        ConnectionStatus()

    /** The transport dropped, but the session is reconnecting within its budget. */
    class Recovering(
        override val deviceManager: DeviceConnectionManager,
        override val macAddress: String,
        val attempt: Int,
        val reason: SessionFailureReason,
    ) : ConnectionStatus()

    /** The session exhausted its recovery budget or hit a terminal cause. */
    class Lost(
        override val deviceManager: DeviceConnectionManager?,
        override val macAddress: String,
        val reason: SessionFailureReason,
    ) : ConnectionStatus()

    /** Only for an explicit disconnect, unpair or service destroy. */
    data object Disconnected : ConnectionStatus() {
        override val macAddress: String? = null
        override val deviceManager: DeviceConnectionManager? = null
    }
}

/**
 * Whether this state already belongs to [macAddress]. Connecting, controlling and recovering are one session: a
 * second request must not close and reopen the native connection. Lost is not, because it may be retried.
 */
internal fun ConnectionStatus.isSameSession(macAddress: String): Boolean = when (this) {
    is ConnectionStatus.Connecting -> this.macAddress.equals(macAddress, ignoreCase = true)
    is ConnectionStatus.Connected -> this.macAddress.equals(macAddress, ignoreCase = true)
    is ConnectionStatus.Recovering -> this.macAddress.equals(macAddress, ignoreCase = true)
    else -> false
}
