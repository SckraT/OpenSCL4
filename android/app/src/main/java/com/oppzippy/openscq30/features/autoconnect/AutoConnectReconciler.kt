package com.oppzippy.openscq30.features.autoconnect

import android.util.Log
import com.oppzippy.openscq30.android.BluetoothProfileDevices
import com.oppzippy.openscq30.android.BluetoothProfiles
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Connects devices that were already connected when autoconnect started: Android does not repeat their ACL broadcast.
 * GATT devices are read on [start]; each classic profile reports once its proxy connects, and the proxy is then
 * released, since devices that connect later arrive through the broadcast. [stop] releases proxies still pending.
 */
internal class AutoConnectReconciler(
    bluetooth: BluetoothProfiles,
    private val coordinator: AutoConnectCoordinator,
    private val scope: CoroutineScope,
) {
    companion object {
        private const val TAG = "AutoConnectReconciler"
    }

    private val profileDevices = BluetoothProfileDevices(
        bluetooth = bluetooth,
        onProfileDevices = ::onProfileDevices,
    )

    fun start() {
        profileDevices.open()
        val gattDevices = try {
            profileDevices.gattDevices()
        } catch (ex: SecurityException) {
            Log.e(TAG, "Missing permission to read connected Bluetooth devices", ex)
            null
        }
        reconcile(gattDevices.orEmpty())
    }

    fun stop() {
        profileDevices.close()
    }

    private fun onProfileDevices(profile: Int, addresses: List<String>) {
        profileDevices.close(profile)
        reconcile(addresses)
    }

    private fun reconcile(addresses: List<String>) {
        scope.launch {
            try {
                coordinator.reconcileConnectedDevices(addresses)
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                Log.e(TAG, "error reconciling connected devices", ex)
            }
        }
    }
}
