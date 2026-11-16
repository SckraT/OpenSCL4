package com.oppzippy.openscq30.features.battery

import android.Manifest
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat
import com.oppzippy.openscq30.android.SystemBattery
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The percentage Android reports for a connected Classic audio device, which is the number its own settings show.
 *
 * Experimental: it comes from a hidden API through [SystemBattery], and Android does not distinguish the earbuds.
 */
@Singleton
class SystemBatteryProvider @Inject constructor(@ApplicationContext private val context: Context) {
    private val loggedMacAddresses = mutableSetOf<String>()

    /** The percentage 0 to 100, or null when Android has no reading, the permission is missing or anything fails. */
    fun percent(macAddress: String): Int? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O_MR1) {
            return null
        }
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            return null
        }
        val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter ?: return null
        if (!adapter.isEnabled) {
            return null
        }
        val device = runCatching { adapter.getRemoteDevice(macAddress) }.getOrNull() ?: return null
        val percent = runCatching { SystemBattery.nativeGetBatteryLevel(device) }
            .getOrNull()
            ?.takeIf { it in 0..100 }
        logOnce(macAddress, percent)
        return percent
    }

    private fun logOnce(macAddress: String, percent: Int?) {
        synchronized(loggedMacAddresses) {
            if (!loggedMacAddresses.add(macAddress)) return
        }
        Log.d(TAG, "mac=$macAddress level=${percent ?: -1}")
    }

    companion object {
        private const val TAG = "SystemBattery"
    }
}
