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
 * Android refuses to start a foreground service from the background unless an exemption applies, such as the companion
 * association or a boot or package-replaced broadcast. A refusal is logged instead of crashing the process;
 * [MainActivity.onResume] calls this again once the app is in the foreground.
 */
internal fun startAutoConnectServiceIfEnabled(
    context: Context,
    preferences: Preferences,
    sdkInt: Int = Build.VERSION.SDK_INT,
) {
    if (!preferences.autoConnect) {
        Log.d(TAG, "not starting the autoconnect service because auto connect is disabled")
        return
    }
    if (!canEnableAutoConnect(context, sdkInt)) {
        // The service would run as a foreground service with no visible notification: exactly the silent background
        // work autoconnect must not do. Opening the app turns autoconnect off in this state and says why.
        Log.w(TAG, "not starting the autoconnect service because notification permission is missing")
        return
    }
    if (AutoConnectService.isRunning) {
        // Every startForegroundService must be answered with startForeground, so starting again would post the
        // notification even while a device is connected. A running service already follows the connection itself.
        Log.d(TAG, "not starting the autoconnect service because it is already running")
        return
    }
    try {
        // minSdk is 26, so the foreground variant is always available.
        context.startForegroundService(autoConnectServiceIntent(context))
    } catch (ex: IllegalStateException) {
        if (!isServiceStartRefusal(ex, sdkInt)) throw ex
        Log.w(TAG, "Android refused to start the autoconnect service; it is retried in the foreground", ex)
    }
}

/**
 * Turns autoconnect off when it is on without the notification permission, and returns whether it did.
 *
 * The permission can be revoked after autoconnect was turned on, and a user who turned it on before it needed the
 * permission may never have granted it. Every start is refused then while the switch still shows autoconnect on, so
 * it is turned off where the user can be told why: when the app is opened.
 */
internal fun turnOffAutoConnectWithoutNotifications(
    context: Context,
    preferences: Preferences,
    sdkInt: Int = Build.VERSION.SDK_INT,
): Boolean {
    if (!preferences.autoConnect || canEnableAutoConnect(context, sdkInt)) return false
    Log.w(TAG, "turning auto connect off because notification permission is missing")
    preferences.autoConnect = false
    stopAutoConnectService(context)
    return true
}

/** [canEnableAutoConnect] with this app's notification permission. */
internal fun canEnableAutoConnect(context: Context, sdkInt: Int = Build.VERSION.SDK_INT): Boolean =
    canEnableAutoConnect(sdkInt, hasNotificationPermission(context))

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

private fun hasNotificationPermission(context: Context): Boolean =
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
