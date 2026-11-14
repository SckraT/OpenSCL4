package com.oppzippy.openscq30.widget

import android.util.Log
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.ui.deviceselection.splitBrand

/** The device name without the manufacturer, as the client's device card shows it. */
internal fun deviceShortName(model: String): String = splitBrand(translateDeviceModel(model)).second

/** A name for the connection status: the device model when it is known, the address otherwise. */
internal fun connectionDeviceName(connectionStatus: ConnectionStatus): String {
    val model = connectionStatus.deviceManager?.let { manager ->
        try {
            deviceShortName(manager.device.model())
        } catch (ex: IllegalStateException) {
            Log.w("WidgetDeviceName", "device was closed, using the address as its name", ex)
            null
        }
    }
    return model ?: connectionStatus.macAddress.orEmpty()
}
