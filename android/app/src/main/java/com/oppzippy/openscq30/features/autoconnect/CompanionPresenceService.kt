package com.oppzippy.openscq30.features.autoconnect

import android.companion.CompanionDeviceService
import android.util.Log
import com.oppzippy.openscq30.features.preferences.Preferences
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

/**
 * Receives companion-device presence events and starts autoconnect when the device appears.
 *
 * The system binds this service itself, which is what makes autoconnect work after a reboot or an app update without
 * the user opening the app. Starting [AutoConnectService] is allowed from here because the app holds an association
 * and declares `REQUEST_COMPANION_START_FOREGROUND_SERVICES_FROM_BACKGROUND`.
 */
@AndroidEntryPoint
class CompanionPresenceService : CompanionDeviceService() {
    companion object {
        private const val TAG = "CompanionPresence"
    }

    @Inject
    lateinit var preferences: Preferences

    override fun onDeviceAppeared(address: String) {
        Log.d(TAG, "companion device appeared")
        if (!preferences.autoConnect) return
        startAutoConnectService(this)
    }
}
