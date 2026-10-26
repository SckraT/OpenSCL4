package com.oppzippy.openscq30.features.autoconnect

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
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
import com.oppzippy.openscq30.MainActivity
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.android.AndroidBluetoothProfiles
import com.oppzippy.openscq30.features.preferences.Preferences
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.launch

/**
 * Waits for a paired device while autoconnect is enabled and starts [DeviceService] when it appears.
 *
 * This service is a foreground service because Android refuses to start a plain background service while the app is
 * in the background, and autoconnect has to work after a reboot or an update. The notification is only shown while no
 * device is connected: once one is, [DeviceService] is foreground and keeps the process alive, and the notification
 * returns as soon as that connection is lost.
 */
@AndroidEntryPoint
class AutoConnectService : LifecycleService() {
    companion object {
        private const val TAG = "AutoConnectService"

        // Must differ from DeviceService.NOTIFICATION_ID: notification ids are shared by the whole app, so a
        // collision would let one service overwrite or remove the other's foreground notification.
        internal const val NOTIFICATION_ID = 2
        const val NOTIFICATION_CHANNEL_ID = "com.oppzippy.openscq30.notification.AutoConnectChannel"

        // True from onCreate to onDestroy, so startAutoConnectServiceIfEnabled can leave a running service alone.
        internal var isRunning = false
            private set
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
        createNotificationChannel()
        observeForegroundNeed()
        isRunning = true
    }

    override fun onDestroy() {
        super.onDestroy()
        application.unregisterReceiver(receiver)
        reconciler.stop()
        isRunning = false
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        // A service started with startForegroundService must call startForeground, or Android kills it. Post the
        // notification first and let updateForeground remove it again when nothing needs to be shown.
        startForeground(NOTIFICATION_ID, buildNotification())
        updateForeground()
        return START_STICKY
    }

    /**
     * The notification is only needed until a device is connected, so it follows [shouldHoldForeground] rather than
     * being posted once and kept. A refused start leaves the service in the background, which the caller handles.
     */
    private fun observeForegroundNeed() {
        lifecycleScope.launch {
            DeviceService.isConnected.collect { deviceConnected ->
                updateForeground(deviceConnected)
            }
        }
    }

    private fun updateForeground(deviceConnected: Boolean = DeviceService.isConnected.value) {
        if (shouldHoldForeground(preferences.autoConnect, deviceConnected)) {
            startForeground(NOTIFICATION_ID, buildNotification())
        } else {
            stopForeground(STOP_FOREGROUND_REMOVE)
        }
    }

    private fun buildNotification() = Notification.Builder(this, NOTIFICATION_CHANNEL_ID).setOngoing(true)
        .setOnlyAlertOnce(true).setSmallIcon(R.drawable.openscq30_logo_foreground).setContentTitle(
            getString(R.string.auto_connect_waiting_for_device),
        ).setContentText(getString(R.string.auto_connect_waiting_for_device_description)).setContentIntent(
            PendingIntent.getActivity(
                this,
                0,
                Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        ).build()

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            getString(R.string.auto_connect_notification_channel),
            NotificationManager.IMPORTANCE_LOW,
        )
        channel.enableVibration(false)
        channel.enableLights(false)
        channel.setSound(null, null)
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
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
