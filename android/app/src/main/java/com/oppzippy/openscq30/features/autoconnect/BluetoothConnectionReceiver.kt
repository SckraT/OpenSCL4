package com.oppzippy.openscq30.features.autoconnect

import android.bluetooth.BluetoothA2dp
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothHeadset
import android.bluetooth.BluetoothProfile
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.util.Log
import androidx.core.content.IntentCompat
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Hands [AutoConnectService] its Bluetooth broadcasts: a device's ACL connection, and an A2DP or HEADSET profile that
 * connected.
 *
 * The profile broadcasts matter when a companion presence event starts the service: the device's ACL broadcast went
 * out before this receiver was registered, and its profiles may come up only after the start reconcile read them.
 */
internal class BluetoothConnectionReceiver(
    private val coroutineScope: CoroutineScope,
    private val coordinator: AutoConnectCoordinator,
) : BroadcastReceiver() {
    companion object {
        private const val TAG = "BluetoothConnectionReceiver"

        private val PROFILE_ACTIONS = setOf(
            BluetoothA2dp.ACTION_CONNECTION_STATE_CHANGED,
            BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED,
        )

        /** The broadcasts this receiver handles. */
        fun intentFilter() = IntentFilter(BluetoothDevice.ACTION_ACL_CONNECTED).apply {
            PROFILE_ACTIONS.forEach(::addAction)
        }
    }

    override fun onReceive(context: Context?, intent: Intent?) {
        val action = intent?.action ?: return
        val isProfileConnected = action in PROFILE_ACTIONS &&
            intent.getIntExtra(BluetoothProfile.EXTRA_STATE, BluetoothProfile.STATE_DISCONNECTED) ==
            BluetoothProfile.STATE_CONNECTED
        if (action != BluetoothDevice.ACTION_ACL_CONNECTED && !isProfileConnected) return
        val device = IntentCompat.getParcelableExtra(
            intent,
            BluetoothDevice.EXTRA_DEVICE,
            BluetoothDevice::class.java,
        ) ?: return

        coroutineScope.launch {
            try {
                if (isProfileConnected) {
                    coordinator.handleConnectedProfile(device.address)
                } else {
                    coordinator.handleConnectedDevice(device.address)
                }
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                Log.e(TAG, "error auto connecting to ${device.address}", ex)
            }
        }
    }
}
