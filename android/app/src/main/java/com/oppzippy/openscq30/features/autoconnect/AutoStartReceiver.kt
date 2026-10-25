package com.oppzippy.openscq30.features.autoconnect

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.oppzippy.openscq30.features.preferences.Preferences
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent

/**
 * Starts the autoconnect service when the device boots and when the package is replaced. Android refuses the start
 * while the process is in the background; the refusal is logged and `MainActivity.onResume` retries, see
 * [startAutoConnectServiceIfEnabled].
 */
class AutoStartReceiver : BroadcastReceiver() {
    companion object {
        /** The actions this receiver handles, matching its intent filter in `AndroidManifest.xml`. */
        private val HANDLED_ACTIONS = setOf(Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED)
    }

    @EntryPoint
    @InstallIn(SingletonComponent::class)
    interface AutoStartHiltEntryPoint {
        fun preferences(): Preferences
    }

    override fun onReceive(context: Context?, intent: Intent?) {
        if (context == null || intent?.action !in HANDLED_ACTIONS) return
        val applicationContext = context.applicationContext
        val preferences = EntryPointAccessors
            .fromApplication<AutoStartHiltEntryPoint>(applicationContext)
            .preferences()
        startAutoConnectServiceIfEnabled(applicationContext, preferences)
    }
}
