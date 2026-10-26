package com.oppzippy.openscq30.features.autoconnect

/**
 * Whether [AutoConnectService] should hold a foreground notification right now.
 *
 * The autoconnect service only needs the foreground while it waits for a device: once the device service runs, its
 * own foreground notification keeps the process alive, so a second permanent notification would be noise.
 */
internal fun shouldHoldForeground(autoConnectEnabled: Boolean, deviceServiceRunning: Boolean): Boolean =
    autoConnectEnabled && !deviceServiceRunning

/**
 * Whether autoconnect may be enabled at all.
 *
 * Holding the autoconnect service in the foreground needs a visible notification, and from Android 13 that needs the
 * notification permission. Without it the service could not run in the background, so enabling autoconnect would
 * promise behaviour the app cannot deliver.
 */
internal fun canEnableAutoConnect(sdkInt: Int, notificationsGranted: Boolean): Boolean =
    sdkInt < NOTIFICATION_PERMISSION_SDK || notificationsGranted

internal const val NOTIFICATION_PERMISSION_SDK = 33
