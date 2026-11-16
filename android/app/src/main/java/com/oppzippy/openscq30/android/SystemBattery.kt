package com.oppzippy.openscq30.android

import android.bluetooth.BluetoothDevice

/**
 * The hidden [BluetoothDevice.getBatteryLevel], which only native code may call.
 *
 * Android's public SDK has no battery reading for a connected Classic audio device: reflection on the hidden
 * method is blocked on current releases, while a JNI call still works. This is experimental and may stop working
 * on a future Android release or on an OEM build, so callers must fall back to the device's own levels.
 */
object SystemBattery {
    init {
        System.loadLibrary("openscq30_android")
    }

    /** The percentage 0 to 100, or -1 when Android has no reading for this device. */
    @JvmStatic
    external fun nativeGetBatteryLevel(device: BluetoothDevice): Int
}
