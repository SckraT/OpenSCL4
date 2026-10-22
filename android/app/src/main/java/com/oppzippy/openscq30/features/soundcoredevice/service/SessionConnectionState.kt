package com.oppzippy.openscq30.features.soundcoredevice.service

/** The durable native session state, independent of the transport status. */
sealed class SessionConnectionState {
    data object Connecting : SessionConnectionState()
    data object Connected : SessionConnectionState()
    data class Recovering(val attempt: Int, val reason: String) : SessionConnectionState()
    data class Lost(val reason: String) : SessionConnectionState()
    data object Closed : SessionConnectionState()
}
