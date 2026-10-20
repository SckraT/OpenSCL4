package com.oppzippy.openscq30.features.autoconnect

import android.Manifest
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.companion.CompanionDeviceManager
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import com.oppzippy.openscq30.features.preferences.Preferences
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

@AndroidEntryPoint
class AutoConnectService : LifecycleService() {
    companion object {
        private const val TAG = "AutoConnectService"

        private val connectedProfiles = listOf(
            BluetoothProfile.GATT,
            BluetoothProfile.GATT_SERVER,
            BluetoothProfile.A2DP,
            BluetoothProfile.HEADSET,
        )
    }

    @Inject
    lateinit var preferences: Preferences

    @Inject
    lateinit var session: OpenScq30Session

    private lateinit var receiver: BluetoothConnectionReceiver

    override fun onCreate() {
        super.onCreate()
        val coordinator = AutoConnectCoordinator(
            isAutoConnectEnabled = { preferences.autoConnect },
            associatedAddresses = ::associatedAddresses,
            savedDevices = { session.pairedDevices() },
            connect = ::startDeviceService,
        )
        receiver = BluetoothConnectionReceiver(
            coroutineScope = lifecycleScope,
            coordinator = coordinator,
        )
        application.registerReceiver(
            receiver,
            IntentFilter(BluetoothDevice.ACTION_ACL_CONNECTED),
        )
        lifecycleScope.launch {
            try {
                coordinator.reconcileConnectedDevices(::connectedDeviceAddresses)
            } catch (ex: CancellationException) {
                throw ex
            } catch (ex: Exception) {
                Log.e(TAG, "error reconciling connected devices", ex)
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        application.unregisterReceiver(receiver)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        return START_STICKY
    }

    private fun associatedAddresses(): List<String> =
        getSystemService(CompanionDeviceManager::class.java)?.associations.orEmpty()

    private fun connectedDeviceAddresses(): List<String> {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            ActivityCompat.checkSelfPermission(
                this,
                Manifest.permission.BLUETOOTH_CONNECT,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.e(TAG, "Missing BLUETOOTH_CONNECT permission")
            return emptyList()
        }

        if (Build.VERSION.SDK_INT in Build.VERSION_CODES.Q..Build.VERSION_CODES.R &&
            ActivityCompat.checkSelfPermission(
                this,
                Manifest.permission.ACCESS_FINE_LOCATION,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.e(TAG, "Missing ACCESS_FINE_LOCATION permission on Android 10-11")
            return emptyList()
        }

        val bluetoothManager = getSystemService(BluetoothManager::class.java)
        if (bluetoothManager == null) {
            Log.e(TAG, "BluetoothManager is null. Does the system not support bluetooth?")
            return emptyList()
        }

        return try {
            connectedProfiles
                .flatMap { profile ->
                    try {
                        bluetoothManager.getConnectedDevices(profile)
                    } catch (ex: IllegalArgumentException) {
                        Log.w(TAG, "Bluetooth profile $profile is not supported", ex)
                        emptyList()
                    }
                }
                .map { it.address }
                .distinctBy { it.lowercase() }
        } catch (ex: SecurityException) {
            Log.e(TAG, "Missing permission to read connected Bluetooth devices", ex)
            emptyList()
        }
    }

    private fun startDeviceService(macAddress: String) {
        Log.d(TAG, "auto connecting to $macAddress")
        val serviceIntent = Intent(this, DeviceService::class.java)
        serviceIntent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, macAddress)
        startForegroundService(serviceIntent)
    }
}
