package com.oppzippy.openscq30.lib.wrapper

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ConnectionDescriptor(val name: String, val macAddress: String)

@Serializable
enum class ConnectionStatus {
    @SerialName("connected")
    Connected,

    @SerialName("disconnected")
    Disconnected,
}
