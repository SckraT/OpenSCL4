package com.oppzippy.openscq30.features.phonelink

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import com.oppzippy.openscq30.android.AndroidBluetoothProfiles
import com.oppzippy.openscq30.android.BluetoothProfileDevices
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.StateFlow

@Singleton
class AndroidPhoneLinkMonitor @Inject constructor(@ApplicationContext private val context: Context) :
    PhoneLinkMonitor {
    private val bluetoothManager: BluetoothManager? = context.getSystemService(BluetoothManager::class.java)
    private val core = PhoneLinkMonitorCore()

    private val profileDevices = BluetoothProfileDevices(
        bluetooth = AndroidBluetoothProfiles(context),
        onProfileDevices = core::onProfileDevices,
        onProfileDisconnected = core::onProfileDisconnected,
    )

    @Volatile
    private var started = false

    override val snapshot: StateFlow<PhoneLinkSignals> = core.snapshot

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                BluetoothDevice.ACTION_ACL_CONNECTED -> {
                    val address = intent.deviceAddress() ?: return
                    core.onAclConnected(address)
                }

                BluetoothDevice.ACTION_ACL_DISCONNECTED -> {
                    val address = intent.deviceAddress() ?: return
                    core.onAclDisconnected(address)
                }

                BluetoothAdapter.ACTION_STATE_CHANGED -> onAdapterStateChanged(intent)
            }
        }
    }

    override fun start() {
        if (started) return
        core.start()
        ContextCompat.registerReceiver(
            context,
            receiver,
            IntentFilter().apply {
                addAction(BluetoothDevice.ACTION_ACL_CONNECTED)
                addAction(BluetoothDevice.ACTION_ACL_DISCONNECTED)
                addAction(BluetoothAdapter.ACTION_STATE_CHANGED)
            },
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        started = true
        profileDevices.open()
        refresh()
    }

    override fun stop() {
        if (!started) return
        started = false
        try {
            context.unregisterReceiver(receiver)
        } catch (_: IllegalArgumentException) {
        }
        profileDevices.close()
        core.stop()
    }

    override fun refresh() {
        if (!started) return
        profileDevices.reread()
        val scan = scan()
        core.applyScan(scan.access, scan.profileAddresses)
    }

    private fun onAdapterStateChanged(intent: Intent) {
        when (intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)) {
            BluetoothAdapter.STATE_OFF -> core.onAdapterOff()

            BluetoothAdapter.STATE_ON -> {
                // Android dropped the profile proxies when the adapter turned off; replace them before scanning.
                core.onAdapterOn()
                profileDevices.open()
                refresh()
            }
        }
    }

    @SuppressLint("MissingPermission")
    private fun scan(): PhoneLinkScan {
        val manager = bluetoothManager ?: return PhoneLinkScan(BluetoothAccess.Unsupported, emptySet())
        val adapter = manager.adapter ?: return PhoneLinkScan(BluetoothAccess.Unsupported, emptySet())
        if (!hasBluetoothPermission()) return PhoneLinkScan(BluetoothAccess.PermissionDenied, emptySet())
        val enabled = try {
            adapter.isEnabled
        } catch (_: SecurityException) {
            return PhoneLinkScan(BluetoothAccess.PermissionDenied, emptySet())
        }
        if (!enabled) return PhoneLinkScan(BluetoothAccess.Off, emptySet())

        val gattDevices = try {
            profileDevices.gattDevices()
        } catch (_: SecurityException) {
            return PhoneLinkScan(BluetoothAccess.PermissionDenied, emptySet())
        }
        if (gattDevices == null) return PhoneLinkScan(BluetoothAccess.Unknown, emptySet())
        return PhoneLinkScan(BluetoothAccess.Ready, gattDevices.toSet())
    }

    private fun hasBluetoothPermission(): Boolean {
        val permission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            Manifest.permission.BLUETOOTH_CONNECT
        } else {
            Manifest.permission.BLUETOOTH
        }
        return ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
    }

    @SuppressLint("MissingPermission")
    private fun Intent.deviceAddress(): String? = try {
        IntentCompat.getParcelableExtra(this, BluetoothDevice.EXTRA_DEVICE, BluetoothDevice::class.java)?.address
    } catch (_: SecurityException) {
        null
    }
}

private data class PhoneLinkScan(val access: BluetoothAccess, val profileAddresses: Set<String>)
