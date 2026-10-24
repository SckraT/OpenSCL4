package com.oppzippy.openscq30.features.autoconnect

import android.bluetooth.BluetoothDevice
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.IntentCompat
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

internal class BluetoothConnectionReceiver(
    private val coroutineScope: CoroutineScope,
    private val coordinator: AutoConnectCoordinator,
) : BroadcastReceiver() {
    companion object {
        private const val TAG = "BluetoothConnectionReceiver"
    }

    override fun onReceive(context: Context?, intent: Intent?) {
        if (intent?.action != BluetoothDevice.ACTION_ACL_CONNECTED) return
        val device = IntentCompat.getParcelableExtra(
            intent,
            BluetoothDevice.EXTRA_DEVICE,
            BluetoothDevice::class.java,
        ) ?: return

        coroutineScope.launch {
            try {
                coordinator.handleConnectedDevice(device.address)
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                Log.e(TAG, "error auto connecting to ${device.address}", ex)
            }
        }
    }
}
