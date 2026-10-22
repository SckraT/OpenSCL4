package com.oppzippy.openscq30.features.soundcoredevice.service

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
        val reason: String,
    ) : ConnectionStatus()

    /** The session exhausted its recovery budget or hit a terminal cause. */
    class Lost(
        override val deviceManager: DeviceConnectionManager?,
        override val macAddress: String,
        val reason: String,
    ) : ConnectionStatus()

    /** Only for an explicit disconnect, unpair or service destroy. */
    data object Disconnected : ConnectionStatus() {
        override val macAddress: String? = null
        override val deviceManager: DeviceConnectionManager? = null
    }
}
