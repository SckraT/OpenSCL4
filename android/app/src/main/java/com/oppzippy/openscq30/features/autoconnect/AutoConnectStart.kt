package com.oppzippy.openscq30.features.autoconnect

import android.app.BackgroundServiceStartNotAllowedException
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import com.oppzippy.openscq30.features.preferences.Preferences

private const val TAG = "AutoConnectStart"

/**
 * Starts [AutoConnectService] when autoconnect is enabled, and does nothing when it is disabled.
 *
 * Android refuses to start a background service while the app is in the background, for example when the process
 * starts for a boot or package-replaced broadcast, or behind the lock screen. That refusal is logged instead of
 * crashing the process; [MainActivity.onResume] calls this again once the app is in the foreground.
 */
internal fun startAutoConnectServiceIfEnabled(context: Context, preferences: Preferences) {
    if (!preferences.autoConnect) {
        Log.d(TAG, "not starting the autoconnect service because auto connect is disabled")
        return
    }
    startAutoConnectService(context)
}

/**
 * Starts [AutoConnectService] for a caller that has already decided autoconnect is on, so it does not re-read the
 * preference. A start that Android refuses in the background is logged instead of crashing the process.
 */
internal fun startAutoConnectService(context: Context) {
    try {
        context.startService(autoConnectServiceIntent(context))
    } catch (ex: IllegalStateException) {
        if (!isBackgroundStartRefusal(ex, Build.VERSION.SDK_INT)) throw ex
        Log.w(TAG, "Android refused to start the autoconnect service; it is retried in the foreground", ex)
    }
}

internal fun stopAutoConnectService(context: Context) {
    context.stopService(autoConnectServiceIntent(context))
}

private fun autoConnectServiceIntent(context: Context) = Intent(context, AutoConnectService::class.java)

/**
 * Whether [exception] from `startService` is Android refusing to start a service from the background.
 *
 * From Android 12 the refusal is [BackgroundServiceStartNotAllowedException], the only [IllegalStateException] that
 * `startService` raises. A sibling [android.app.ForegroundServiceStartNotAllowedException] cannot come from
 * `startService` at all, so checking this subclass rather than the shared superclass keeps other failures loud.
 *
 * Before Android 12 the refusal is a bare [IllegalStateException], and it is the only [IllegalStateException] that
 * `startService` raises, so all of them are refusals there. Testing the exact class instead would let a ROM that
 * subclasses it crash the process again, which is the bug this guards.
 */
internal fun isBackgroundStartRefusal(exception: IllegalStateException, sdkInt: Int): Boolean =
    sdkInt < Build.VERSION_CODES.S || exception is BackgroundServiceStartNotAllowedException
