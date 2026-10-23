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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

@Singleton
class AndroidPhoneLinkMonitor @Inject constructor(@ApplicationContext private val context: Context) :
    PhoneLinkMonitor {
    private val bluetoothManager: BluetoothManager? = context.getSystemService(BluetoothManager::class.java)
    private val state = MutableStateFlow(PhoneLinkSignals())
    @Volatile
    private var started = false
    private val classicDevices = ConcurrentHashMap<Int, Set<String>>()
    private val classicProxies = ConcurrentHashMap<Int, BluetoothProfile>()

    private val serviceListener = object : BluetoothProfile.ServiceListener {
        override fun onServiceConnected(profile: Int, proxy: BluetoothProfile?) {
            if (proxy == null) return
            classicProxies[profile] = proxy
            updateClassicDeviceAddresses(profile, proxy)
        }

        override fun onServiceDisconnected(profile: Int) {
            classicProxies.remove(profile)
            classicDevices.remove(profile)
            refresh()
        }
    }

    override val snapshot: StateFlow<PhoneLinkSignals> = state.asStateFlow()

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                BluetoothDevice.ACTION_ACL_CONNECTED -> {
                    val address = intent.deviceAddress() ?: return
                    state.update { it.copy(aclAddresses = it.aclAddresses + address) }
                }

                BluetoothDevice.ACTION_ACL_DISCONNECTED -> {
                    val address = intent.deviceAddress() ?: return
                    state.update { it.copy(aclAddresses = it.aclAddresses - address) }
                }

                BluetoothAdapter.ACTION_STATE_CHANGED -> onAdapterStateChanged(intent)
            }
        }
    }

    @SuppressLint("MissingPermission")
    override fun start() {
        if (started) return
        state.value = PhoneLinkSignals()
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
        classicDevices.clear()
        state.update { it.copy(aclAddresses = emptySet()) }
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
            proxy.connectedDevices.map { canonicalMacAddress(it.address) }.toSet()
        } catch (_: SecurityException) {
            return
        }
        classicDevices[profile] = addresses
        refresh()
    }

    override fun refresh() {
        state.value = scan(state.value.aclAddresses)
    }

    private fun onAdapterStateChanged(intent: Intent) {
        when (intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)) {
            BluetoothAdapter.STATE_OFF -> state.value = PhoneLinkSignals(BluetoothAccess.Off)
            BluetoothAdapter.STATE_ON -> refresh()
        }
    }

    @SuppressLint("MissingPermission")
    private fun scan(aclAddresses: Set<String>): PhoneLinkSignals {
        val manager = bluetoothManager ?: return PhoneLinkSignals(BluetoothAccess.Unsupported)
        val adapter = manager.adapter ?: return PhoneLinkSignals(BluetoothAccess.Unsupported)
        if (!hasBluetoothPermission()) return PhoneLinkSignals(BluetoothAccess.PermissionDenied)
        val enabled = try {
            adapter.isEnabled
        } catch (_: SecurityException) {
            return PhoneLinkSignals(BluetoothAccess.PermissionDenied)
        }
        if (!enabled) return PhoneLinkSignals(BluetoothAccess.Off)

        val addresses = mutableSetOf<String>()
        classicDevices.values.forEach { addresses += it }
        var readableProfiles = 0
        for (profile in PROFILES) {
            try {
                val devices = manager.getConnectedDevices(profile)
                devices.forEach { addresses += canonicalMacAddress(it.address) }
                readableProfiles++
            } catch (_: IllegalArgumentException) {
            } catch (_: SecurityException) {
                return PhoneLinkSignals(BluetoothAccess.PermissionDenied)
            }
        }
        val access = if (readableProfiles == 0) BluetoothAccess.Unknown else BluetoothAccess.Ready
        return PhoneLinkSignals(access, profileAddresses = addresses, aclAddresses = aclAddresses)
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
