package com.oppzippy.openscq30.features.autoconnect

import android.companion.CompanionDeviceManager
import android.companion.DeviceNotAssociatedException
import android.content.Context
import android.util.Log

private const val TAG = "AutoConnectPresence"

/**
 * Asks the system to report when each associated companion device appears, and returns how many were registered.
 *
 * Registration is best effort: an association can be removed between reading the list and registering it, and the
 * permission to observe presence is optional, so one failure must not stop the remaining devices.
 */
internal fun observeCompanionPresence(addresses: List<String>, startObserving: (String) -> Unit): Int {
    var observed = 0
    for (address in addresses) {
        try {
            startObserving(address)
            observed++
        } catch (ex: DeviceNotAssociatedException) {
            Log.w(TAG, "device is no longer associated, skipping presence observation", ex)
        } catch (ex: SecurityException) {
            Log.w(TAG, "missing permission to observe device presence, skipping", ex)
        }
    }
    return observed
}

/** Registers presence observation for every device already associated with this app. */
internal fun observeAssociatedDevicePresence(context: Context) {
    val manager = context.getSystemService(CompanionDeviceManager::class.java) ?: return
    val addresses = try {
        manager.associations.orEmpty()
    } catch (ex: SecurityException) {
        Log.w(TAG, "missing permission to read companion associations", ex)
        return
    }
    observeCompanionPresence(addresses) { address -> manager.startObservingDevicePresence(address) }
}
