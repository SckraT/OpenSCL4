package com.oppzippy.openscq30.features.phonelink

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.flow.StateFlow

@Singleton
class AndroidPhoneLinkMonitor @Inject constructor(@ApplicationContext private val context: Context) :
    PhoneLinkMonitor {
    private val bluetoothManager: BluetoothManager? = context.getSystemService(BluetoothManager::class.java)
    private val core = PhoneLinkMonitorCore()

    @Volatile
    private var started = false
    private val classicProxies = ConcurrentHashMap<Int, BluetoothProfile>()

    private val serviceListener = object : BluetoothProfile.ServiceListener {
        override fun onServiceConnected(profile: Int, proxy: BluetoothProfile?) {
            if (proxy == null) return
            classicProxies[profile] = proxy
            updateClassicDeviceAddresses(profile, proxy)
        }

        override fun onServiceDisconnected(profile: Int) {
            classicProxies.remove(profile)
            core.onProfileDisconnected(profile)
            // Android drops the proxy with the adapter; re-read the profile when it is available again.
            if (started) openClassicProxies()
        }
    }

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

    @SuppressLint("MissingPermission")
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
        openClassicProxies()
        refresh()
    }

    override fun stop() {
        if (!started) return
        started = false
        try {
            context.unregisterReceiver(receiver)
        } catch (_: IllegalArgumentException) {
        }
        val adapter = bluetoothManager?.adapter
        for ((profile, proxy) in classicProxies) {
            try {
                adapter?.closeProfileProxy(profile, proxy)
            } catch (_: SecurityException) {
            }
        }
        classicProxies.clear()
        core.stop()
    }

    @SuppressLint("MissingPermission")
    private fun openClassicProxies() {
        val adapter = bluetoothManager?.adapter ?: return
        for (profile in CLASSIC_PROFILES) {
            try {
                adapter.getProfileProxy(context, serviceListener, profile)
            } catch (_: SecurityException) {
            }
        }
    }

    @SuppressLint("MissingPermission")
    private fun updateClassicDeviceAddresses(profile: Int, proxy: BluetoothProfile) {
        if (!started) return
        val addresses = try {
            proxy.connectedDevices.map { it.address }
        } catch (_: SecurityException) {
            return
        }
        core.onProfileDevices(profile, addresses)
    }

    @SuppressLint("MissingPermission")
    private fun rereadClassicDeviceAddresses() {
        for ((profile, proxy) in classicProxies) {
            updateClassicDeviceAddresses(profile, proxy)
        }
    }

    override fun refresh() {
        if (!started) return
        rereadClassicDeviceAddresses()
        val scan = scan()
        core.applyScan(scan.access, scan.profileAddresses)
    }

    private fun onAdapterStateChanged(intent: Intent) {
        when (intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)) {
            BluetoothAdapter.STATE_OFF -> core.onAdapterOff()

            BluetoothAdapter.STATE_ON -> {
                // The adapter closed its profile proxies when it turned off; open and re-read them before scanning.
                core.onAdapterOn()
                openClassicProxies()
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

        val addresses = mutableSetOf<String>()
        var readableProfiles = 0
        for (profile in PROFILES) {
            try {
                val devices = manager.getConnectedDevices(profile)
                devices.mapTo(addresses) { canonicalMacAddress(it.address) }
                readableProfiles++
            } catch (_: IllegalArgumentException) {
            } catch (_: SecurityException) {
                return PhoneLinkScan(BluetoothAccess.PermissionDenied, emptySet())
            }
        }
        val access = if (readableProfiles == 0) BluetoothAccess.Unknown else BluetoothAccess.Ready
        return PhoneLinkScan(access, addresses)
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

    private companion object {
        val PROFILES = listOf(
            BluetoothProfile.GATT,
            BluetoothProfile.GATT_SERVER,
        )
        val CLASSIC_PROFILES = listOf(
            BluetoothProfile.A2DP,
            BluetoothProfile.HEADSET,
        )
    }
}

private data class PhoneLinkScan(val access: BluetoothAccess, val profileAddresses: Set<String>)
