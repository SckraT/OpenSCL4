package com.oppzippy.openscq30.features.autoconnect

import android.os.Build

/**
 * Whether [AutoConnectService] should hold a foreground notification right now.
 *
 * The autoconnect service only needs the foreground while it has not connected a device yet. A running device
 * service is not enough: it exists before the connection is up and stays through a loss, so the handover follows
 * [deviceConnected] rather than "some other service is running".
 */
internal fun shouldHoldForeground(autoConnectEnabled: Boolean, deviceConnected: Boolean): Boolean =
    autoConnectEnabled && !deviceConnected

/**
 * Whether autoconnect may be on, and may keep starting in the background.
 *
 * The autoconnect service is a foreground service, and a foreground service without a visible notification runs
 * hidden from the user. Without the notification permission that is what autoconnect would become, so it stays off
 * instead: the settings switch refuses to turn it on, and [turnOffAutoConnectWithoutNotifications] turns it off when
 * the permission is missing later.
 */
internal fun canEnableAutoConnect(sdkInt: Int, notificationsGranted: Boolean): Boolean =
    sdkInt < Build.VERSION_CODES.TIRAMISU || notificationsGranted
