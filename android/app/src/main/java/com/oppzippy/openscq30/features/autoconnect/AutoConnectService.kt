package com.oppzippy.openscq30.features.autoconnect

import android.Manifest
import android.bluetooth.BluetoothDevice
import android.companion.CompanionDeviceManager
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import com.oppzippy.openscq30.android.AndroidBluetoothProfiles
import com.oppzippy.openscq30.features.preferences.Preferences
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class AutoConnectService : LifecycleService() {
    companion object {
        private const val TAG = "AutoConnectService"
    }

    @Inject
    lateinit var preferences: Preferences

    @Inject
    lateinit var session: OpenScq30Session

    private lateinit var receiver: BluetoothConnectionReceiver
    private lateinit var reconciler: AutoConnectReconciler

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
        reconciler = AutoConnectReconciler(
            bluetooth = AndroidBluetoothProfiles(applicationContext),
            coordinator = coordinator,
            scope = lifecycleScope,
        )
        if (canReadConnectedDevices()) reconciler.start()
    }

    override fun onDestroy() {
        super.onDestroy()
        application.unregisterReceiver(receiver)
        reconciler.stop()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        return START_STICKY
    }

    private fun associatedAddresses(): List<String> =
        getSystemService(CompanionDeviceManager::class.java)?.associations.orEmpty()

    private fun canReadConnectedDevices(): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            ActivityCompat.checkSelfPermission(
                this,
                Manifest.permission.BLUETOOTH_CONNECT,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.e(TAG, "Missing BLUETOOTH_CONNECT permission")
            return false
        }

        if (Build.VERSION.SDK_INT in Build.VERSION_CODES.Q..Build.VERSION_CODES.R &&
            ActivityCompat.checkSelfPermission(
                this,
                Manifest.permission.ACCESS_FINE_LOCATION,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.e(TAG, "Missing ACCESS_FINE_LOCATION permission on Android 10-11")
            return false
        }
        return true
    }

    private fun startDeviceService(macAddress: String) {
        Log.d(TAG, "auto connecting to $macAddress")
        val serviceIntent = Intent(this, DeviceService::class.java)
        serviceIntent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, macAddress)
        startForegroundService(serviceIntent)
    }
}
