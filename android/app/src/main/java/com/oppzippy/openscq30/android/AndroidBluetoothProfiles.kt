package com.oppzippy.openscq30.android

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.Context

/** [BluetoothProfiles] on the Android framework. Callers check the Bluetooth permissions first. */
internal class AndroidBluetoothProfiles(private val context: Context) : BluetoothProfiles {
    private val manager: BluetoothManager? = context.getSystemService(BluetoothManager::class.java)

    override fun openProxy(profile: Int, onConnected: (ProfileProxy) -> Unit, onDisconnected: () -> Unit): Boolean {
        val adapter = manager?.adapter ?: return false
        val listener = object : BluetoothProfile.ServiceListener {
            override fun onServiceConnected(profile: Int, proxy: BluetoothProfile?) {
                if (proxy != null) onConnected(AndroidProfileProxy(adapter, profile, proxy))
            }

            override fun onServiceDisconnected(profile: Int) = onDisconnected()
        }
        return adapter.getProfileProxy(context, listener, profile)
    }

    @SuppressLint("MissingPermission")
    override fun connectedDevices(profile: Int): List<String> =
        manager?.getConnectedDevices(profile)?.map { it.address }.orEmpty()
}

private class AndroidProfileProxy(
    private val adapter: BluetoothAdapter,
    private val profile: Int,
    private val proxy: BluetoothProfile,
) : ProfileProxy {
    @SuppressLint("MissingPermission")
    override fun connectedDevices(): List<String> = proxy.connectedDevices.map { it.address }

    override fun close() {
        try {
            adapter.closeProfileProxy(profile, proxy)
        } catch (_: SecurityException) {
        }
    }
}
