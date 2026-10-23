package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.SessionFailureReason

/** The durable native session state, independent of the transport status. */
sealed class SessionConnectionState {
    data object Connecting : SessionConnectionState()
    data object Connected : SessionConnectionState()
    data class Recovering(val attempt: Int, val reason: SessionFailureReason) : SessionConnectionState()
    data class Lost(val reason: SessionFailureReason) : SessionConnectionState()
    data object Closed : SessionConnectionState()
}
