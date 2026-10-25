package com.oppzippy.openscq30.features.autoconnect

import android.content.Context
import android.content.Intent
import android.util.Log
import com.oppzippy.openscq30.features.preferences.Preferences

private const val TAG = "AutoConnectStart"

/**
 * Starts [AutoConnectService] when autoconnect is enabled. A start that Android refuses in the background is logged
 * instead of crashing the process; MainActivity starts it again once the app is in the foreground.
 */
internal fun startAutoConnectServiceIfEnabled(context: Context, preferences: Preferences) {
    if (!preferences.autoConnect) return
    startUnlessRefused(
        start = { context.startService(autoConnectServiceIntent(context)) },
        onRefused = { Log.w(TAG, "Android refused to start the autoconnect service", it) },
    )
}

internal fun stopAutoConnectService(context: Context) {
    context.stopService(autoConnectServiceIntent(context))
}

private fun autoConnectServiceIntent(context: Context) = Intent(context, AutoConnectService::class.java)

/**
 * Runs [start] unless Android refuses it. Android does not start a background service while the app is in the
 * background, for example when the process starts for a boot or package-replaced broadcast or behind the lock screen
 * (`BackgroundServiceStartNotAllowedException`, a plain [IllegalStateException] before Android 12). That refusal goes
 * to [onRefused] instead of crashing the process.
 */
internal fun startUnlessRefused(start: () -> Unit, onRefused: (IllegalStateException) -> Unit): Boolean = try {
    start()
    true
} catch (ex: IllegalStateException) {
    onRefused(ex)
    false
}
