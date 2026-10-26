package com.oppzippy.openscq30.features.autoconnect

import android.app.ServiceStartNotAllowedException
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import com.oppzippy.openscq30.features.preferences.Preferences

private const val TAG = "AutoConnectStart"

/**
 * Starts [AutoConnectService] when autoconnect is enabled, and does nothing when it is disabled.
 *
 * Android refuses to start a foreground service while the app is in the background, for example when the process
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
        // minSdk is 26, so the foreground variant is always available.
        context.startForegroundService(autoConnectServiceIntent(context))
    } catch (ex: IllegalStateException) {
        if (!isBackgroundStartRefusal(ex, Build.VERSION.SDK_INT)) throw ex
        Log.w(TAG, "Android refused to start the autoconnect service; it is retried in the foreground", ex)
    }
}

/**
 * Registers companion-device presence observation, so the system reports when a paired device appears even while the
 * app is not running. Called when autoconnect is turned on and again after boot or an update, since the previous
 * registrations do not survive those.
 */
internal fun observeCompanionPresenceForAutoconnect(context: Context) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
        Log.d(TAG, "device presence observation needs Android 12; autoconnect happens on app start instead")
        return
    }
    observeAssociatedDevicePresence(context.applicationContext)
}

internal fun stopAutoConnectService(context: Context) {
    context.stopService(autoConnectServiceIntent(context))
}

private fun autoConnectServiceIntent(context: Context) = Intent(context, AutoConnectService::class.java)

/**
 * Whether [exception] from starting the autoconnect service is Android refusing to start a service from the
 * background.
 *
 * Starting a foreground service raises [android.app.ForegroundServiceStartNotAllowedException] from Android 12 and
 * [android.app.BackgroundServiceStartNotAllowedException] when a plain service start is refused; both are
 * [ServiceStartNotAllowedException]s. Before Android 12 the refusal is a bare [IllegalStateException], and it is the
 * only [IllegalStateException] that starting this service raises, so all of them are refusals there.
 */
internal fun isBackgroundStartRefusal(exception: IllegalStateException, sdkInt: Int): Boolean =
    if (sdkInt >= Build.VERSION_CODES.S) {
        exception is ServiceStartNotAllowedException
    } else {
        true
    }
