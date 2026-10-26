package com.oppzippy.openscq30.features.autoconnect

import android.Manifest
import android.app.ServiceStartNotAllowedException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat
import com.oppzippy.openscq30.features.preferences.Preferences

private const val TAG = "AutoConnectStart"

/**
 * The single entry point for starting [AutoConnectService]: every caller — boot or package-replaced broadcast,
 * companion presence, app resume and the settings switch — goes through here, so the two preconditions cannot drift
 * apart between them.
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
    if (!canEnableAutoConnect(Build.VERSION.SDK_INT, hasNotificationPermission(context))) {
        // The service would run as a foreground service with no visible notification: exactly the silent background
        // work autoconnect must not do. The settings switch already refuses to enable it in this state.
        Log.w(TAG, "not starting the autoconnect service because notification permission is missing")
        return
    }
    try {
        // minSdk is 26, so the foreground variant is always available.
        context.startForegroundService(autoConnectServiceIntent(context))
    } catch (ex: IllegalStateException) {
        if (!isServiceStartRefusal(ex, Build.VERSION.SDK_INT)) throw ex
        Log.w(TAG, "Android refused to start the autoconnect service; it is retried in the foreground", ex)
    }
}

/**
 * Registers companion-device presence observation, so the system reports when a paired device appears even while the
 * app is not running.
 *
 * Registrations do not survive a reboot or an update, so this is called when autoconnect is turned on, when a device
 * is paired, and again from the boot and package-replaced receiver.
 */
internal fun observeCompanionPresenceForAutoconnect(context: Context) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
        Log.d(TAG, "device presence observation needs Android 12; the boot receiver starts autoconnect instead")
        return
    }
    observeAssociatedDevicePresence(context.applicationContext)
}

internal fun hasNotificationPermission(context: Context): Boolean =
    ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
        PackageManager.PERMISSION_GRANTED

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
internal fun isServiceStartRefusal(exception: IllegalStateException, sdkInt: Int): Boolean =
    if (sdkInt >= Build.VERSION_CODES.S) {
        exception is ServiceStartNotAllowedException
    } else {
        true
    }
