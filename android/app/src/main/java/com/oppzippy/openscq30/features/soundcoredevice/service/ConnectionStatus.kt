package com.oppzippy.openscq30.features.soundcoredevice.service

sealed class ConnectionStatus {
    data object AwaitingConnection : ConnectionStatus()
    class Connecting(val macAddress: String) : ConnectionStatus()
    class Connected(val deviceManager: DeviceConnectionManager, val macAddress: String? = null) : ConnectionStatus()
    data object Disconnected : ConnectionStatus()
}
