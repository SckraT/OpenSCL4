package com.oppzippy.openscq30.ui.deviceselection

import androidx.annotation.StringRes
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.batteryProgress

enum class ActiveDeviceState {
    Connecting,
    Connected,
}

data class DeviceBattery(@StringRes val label: Int, val value: String, val progress: Float?)

/** The device the app is connecting or connected to, with its battery levels once they are known. */
data class ActiveDevice(
    val macAddress: String,
    val state: ActiveDeviceState,
    val batteries: List<DeviceBattery> = emptyList(),
)

/**
 * The device at the top of the list: the one the app is connected or connecting to, otherwise the one the user opened
 * last, otherwise the first saved device.
 */
internal fun featuredDevice(
    devices: List<PairedDevice>,
    activeMacAddress: String?,
    lastMacAddress: String?,
): PairedDevice? = devices.firstOrNull { it.macAddress.equals(activeMacAddress, ignoreCase = true) }
    ?: devices.firstOrNull { it.macAddress.equals(lastMacAddress, ignoreCase = true) }
    ?: devices.firstOrNull()

/** Battery levels of the connected device, in the same order as on the device overview. */
fun deviceBatteries(settings: Map<String, Setting>): List<DeviceBattery> = listOf(
    "batteryLevelLeft" to R.string.sound_overview_left,
    "batteryLevelRight" to R.string.sound_overview_right,
    "caseBatteryLevel" to R.string.sound_overview_case,
    "batteryLevel" to R.string.sound_overview_battery,
).mapNotNull { (id, label) ->
    (settings[id] as? Setting.InformationSetting)?.let {
        DeviceBattery(label, it.translatedValue, batteryProgress(it.value))
    }
}

/** Model names start with the brand, which the list shows separately: "Soundcore Liberty 4 Pro". */
internal fun splitBrand(modelName: String): Pair<String?, String> {
    val brand = "Soundcore "
    return if (modelName.startsWith(brand) && modelName.length > brand.length) {
        brand.trim() to modelName.removePrefix(brand)
    } else {
        null to modelName
    }
}
