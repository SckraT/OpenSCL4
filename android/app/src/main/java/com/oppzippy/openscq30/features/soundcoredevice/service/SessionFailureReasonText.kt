package com.oppzippy.openscq30.features.soundcoredevice.service

import androidx.annotation.StringRes
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason

/** Localized text for a typed session failure, so the UI never renders a raw platform error. */
@StringRes
internal fun SessionFailureReason.textResource(): Int = when (this) {
    SessionFailureReason.TRANSPORT_LOST -> R.string.session_reason_transport_lost
    SessionFailureReason.CONNECTION_STATUS_DISCONNECTED -> R.string.session_reason_connection_status_disconnected
    SessionFailureReason.READ_CHANNEL_CLOSED -> R.string.session_reason_read_channel_closed
    SessionFailureReason.WRITE_FAILED -> R.string.session_reason_write_failed
    SessionFailureReason.WRITE_TIMED_OUT -> R.string.session_reason_write_timed_out
    SessionFailureReason.SILENCE_WATCHDOG_FAILED -> R.string.session_reason_silence_watchdog_failed
    SessionFailureReason.CONNECT_TIMED_OUT -> R.string.session_reason_connect_timed_out
    SessionFailureReason.STATE_REFRESH_FAILED -> R.string.session_reason_state_refresh_failed
    SessionFailureReason.BLUETOOTH_ADAPTER_UNAVAILABLE -> R.string.session_reason_bluetooth_adapter_unavailable
    SessionFailureReason.DEVICE_NOT_FOUND -> R.string.session_reason_device_not_found
    SessionFailureReason.PERMISSION_DENIED -> R.string.session_reason_permission_denied
    SessionFailureReason.UNPAIRED -> R.string.session_reason_unpaired
    SessionFailureReason.SECURITY_FAILURE -> R.string.session_reason_security_failure
    SessionFailureReason.CONNECTION_FAILED -> R.string.session_reason_connection_failed
}
