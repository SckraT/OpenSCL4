package com.oppzippy.openscq30.features.autoconnect

import android.content.Context
import android.content.Intent
import android.util.Log

private const val TAG = "AutoConnectStart"

/** Starts [AutoConnectService]; false when Android refuses a background start, which is logged. */
internal fun startAutoConnectService(context: Context): Boolean = startUnlessRefused(
    start = { context.startService(Intent(context, AutoConnectService::class.java)) },
    onRefused = { Log.w(TAG, "Android refused to start the autoconnect service in the background", it) },
)

/**
 * Runs [start] unless Android refuses it. Android does not start a background service while the app is in the
 * background, for example when the process starts for a boot or package-replaced broadcast or behind the lock screen
 * (`BackgroundServiceStartNotAllowedException`, a plain [IllegalStateException] before Android 12). That refusal goes
 * to [onRefused] instead of crashing the process; the next foreground start retries it.
 */
internal fun startUnlessRefused(start: () -> Unit, onRefused: (IllegalStateException) -> Unit): Boolean = try {
    start()
    true
} catch (ex: IllegalStateException) {
    onRefused(ex)
    false
}
